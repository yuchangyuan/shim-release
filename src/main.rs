use std::{env};
use log::{debug, info, log_enabled, warn, Level};
use sv_parser::SyntaxTree;
use std::collections::{BTreeMap, HashMap, BTreeSet};
use env_logger::Env;

use sv_parser::{parse_sv, unwrap_node, Locate, RefNode, Define, Defines, DefineText};

use crc::{Crc, CRC_32_CKSUM};


#[derive(PartialEq, Clone, Debug)]
struct Parameter {
    file_list: Vec<String>,
    defines: BTreeMap<String, Option<String>>,
    inc_list: Vec<String>,
    top_set: BTreeSet<String>,
    rev: usize,
    pkg: String,
}

enum PNext {
    #[allow(non_camel_case_types)] P_TOP,
    #[allow(non_camel_case_types)] P_REV,
    #[allow(non_camel_case_types)] P_PKG,
    #[allow(non_camel_case_types)] P_NONE
}

use PNext::*;

const PKG_DEFAULT: &'static str = "default";
const REV_DEFAULT: usize = 0;

fn parse_args(args: Vec<String>) -> Parameter {
    let mut file_list: Vec<String> = Vec::new();
    let mut defines: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut inc_list: Vec<String> = Vec::new();
    let mut top_set: BTreeSet<String> = BTreeSet::new();

    let mut rev: usize = REV_DEFAULT;
    let mut pkg: String = PKG_DEFAULT.into();

    let mut pnext: PNext = P_NONE;

    for arg in args.into_iter() {
        match pnext {
            P_NONE => {
                if (arg.len() >= 8) && (&arg[0..8] == "+define+") {
                    let kv = &arg[8..];
                    let (k, v) = match kv.find('=') {
                        None => (kv.to_string(), None),
                        Some(idx) => (kv[0..idx].to_string(), Some(kv[idx+1..].to_string()))
                    };

                    defines.insert(k, v);
                }
                else if (arg.len() > 8) && (&arg[0..8] == "+incdir+") {
                    inc_list.push(arg[8..].to_string());
                }
                else if arg == "-t" { pnext = P_TOP; }
                else if arg == "-r" { pnext = P_REV; }
                else if arg == "-p" { pnext = P_PKG; }
                else {
                    file_list.push(arg)
                }
            },
            P_TOP => {
                top_set.insert(arg);
                pnext = P_NONE;
            },
            P_REV => {
                if rev != REV_DEFAULT { warn!("old revision {} be overrided", rev) }
                rev = arg.parse().unwrap();
                pnext = P_NONE;
            },
            P_PKG => {
                if pkg != PKG_DEFAULT { warn!("old package {} be overrided", pkg) }
                pkg = arg;
                pnext = P_NONE;
            },
        }
    }

    Parameter { file_list, defines, inc_list, top_set, rev, pkg }
}


fn show_info(p: &Parameter) {
    info!("package {}, rev {}", p.pkg, p.rev);
    if p.pkg == PKG_DEFAULT { warn!("package not set, use default '{}'", p.pkg) }
    if p.rev == REV_DEFAULT { warn!("revision not set, use default {}", p.rev) }
    if p.top_set.is_empty() { warn!("top set is empty") }

    if log_enabled!(Level::Debug) {
        debug!("define list:");
        for (k, v) in p.defines.iter() {
            match v {
                None => debug!("  - {}", k),
                Some(v1) => debug!("  - {}={}", k, v1)
            }
        }

        debug!("file list:");
        for f in p.file_list.iter() {
            debug!("  - {}", f);
        }

        debug!("include path list:");
        for i in p.inc_list.iter() {
            debug!("  - {}", i);
        }
    }
}

fn to_defines(defs: &BTreeMap<String, Option<String>>) -> Defines {
    let mut res: Defines = HashMap::new();

    for (k, v) in defs.iter() {
        let v1 = match v {
            None => Define {
                identifier: k.clone(),
                arguments: vec![],
                text: None
            },
            Some(x) => Define {
                identifier: k.clone(),
                arguments: vec![],
                text: Some(DefineText {
                    text: x.to_string(),
                    origin: None
                })
            }
        };

        res.insert(k.to_string(), Some(v1));
    }

    res
}


fn parse_files(p: &Parameter) -> BTreeMap<String, SyntaxTree> {
    info!("parse files");

    let mut res: BTreeMap<String, SyntaxTree> = BTreeMap::new();

    let defines = to_defines(&p.defines);

    for file in p.file_list.iter() {
        info!("  parsing {} ...", file);

        let (syntax_tree, _) = parse_sv(&file, &defines, &p.inc_list, false, false).unwrap();

        res.insert(file.to_string(), syntax_tree);
    }

    res
}

pub const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_CKSUM);

type Loc = (usize, usize, u32);
type FileLoc = (String, usize, usize, u32);

fn rewrite(p: &Parameter, st_map: BTreeMap<String, SyntaxTree>) {
    // do two pass
    let mut module_map: BTreeMap<String, u32> = BTreeMap::new();
    let mut rename_map: BTreeMap<FileLoc, (String, bool)> = BTreeMap::new();

    // ------------- first pass --------------
    info!("rewreite, 1st pass...");
    for (path, syntax_tree) in st_map.iter() {
        let mut whitespace_or_comment: BTreeSet<Loc> = BTreeSet::new();
        let mut curr_module: Option<String> = None;
        let mut curr_digest = CRC32.digest();

        info!("  {} ...", path);

        for node in syntax_tree {
            match node {
                RefNode::Locate(x) => {
                    if whitespace_or_comment.contains(&(x.offset, x.len, x.line)) {
                        continue;
                    }
                    else {
                        let str = syntax_tree.get_str(x).unwrap();
                        curr_digest.update(str.as_bytes());
                    }
                }

                RefNode::WhiteSpace(x) => {
                    if let Some(RefNode::Locate(loc)) = unwrap_node!(x, Locate) {
                        whitespace_or_comment.insert((loc.offset, loc.len, loc.line));
                    }
                }

                RefNode::ModuleInstantiation(x) => {
                    let mid = unwrap_node!(x, ModuleIdentifier).unwrap();
                    let mid_loc = get_identifier(mid).unwrap();
                    let mod_name = syntax_tree.get_str(&mid_loc).unwrap();

                    let iid = unwrap_node!(x, InstanceIdentifier).unwrap();
                    let iid_loc = get_identifier(iid).unwrap();
                    let inst_name = syntax_tree.get_str(&iid_loc).unwrap();

                    rename_map.insert((path.clone(), mid_loc.offset, mid_loc.len, mid_loc.line),
                                      (mod_name.to_string(), false));

                    debug!("      - {}: {}", inst_name, mod_name);
                }

                RefNode::ModuleDeclaration(x) => {
                    if unwrap_node!(x, ModuleDeclarationAnsi, ModuleDeclarationNonansi) != None {
                        let id = unwrap_node!(x, ModuleIdentifier).unwrap();
                        let loc = get_identifier(id).unwrap();
                        let name = syntax_tree.get_str(&loc).unwrap();

                        rename_map.insert((path.clone(), loc.offset, loc.len, loc.line),
                                          (name.to_string(), true));

                        if let Some(m) = curr_module {
                            if module_map.contains_key(&m) { warn!("    module {} redefined", &m); }
                            module_map.insert(m, curr_digest.finalize());
                        }

                        debug!("    module {}", name);

                        curr_module = Some(name.to_string());
                        curr_digest = CRC32.digest();

                        // uniquify by pkg & rev
                        curr_digest.update(p.pkg.as_bytes());
                        curr_digest.update(p.rev.to_string().as_bytes());
                    }
                }

                _ => (),
            }
        }

        if let Some(m) = curr_module {
            module_map.insert(m, curr_digest.finalize());
        }
    }

    // -------------- 2nd pass -------------
    info!("rewreite, 2nd pass...");

}

fn get_identifier(node: RefNode) -> Option<Locate> {
    // unwrap_node! can take multiple types
    match unwrap_node!(node, SimpleIdentifier, EscapedIdentifier) {
        Some(RefNode::SimpleIdentifier(x)) => {
            return Some(x.nodes.0);
        }
        Some(RefNode::EscapedIdentifier(x)) => {
            return Some(x.nodes.0);
        }
        _ => None,
    }
}

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().skip(1).collect();
    let p = parse_args(args);

    show_info(&p);

    let syntax_tree_map = parse_files(&p);
    rewrite(&p, syntax_tree_map);
}
