use std::{env};
use log::{debug, info, log_enabled, warn, Level};
use sv_parser::SyntaxTree;
use std::collections::{BTreeMap, HashMap};
use env_logger::Env;

use sv_parser::{parse_sv, Define, Defines, DefineText};

#[derive(PartialEq, Clone, Debug)]
struct Parameter {
    file_list: Vec<String>,
    defines: BTreeMap<String, Option<String>>,
    inc_list: Vec<String>,
    top_list: Vec<String>,
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
    let mut top_list: Vec<String> = vec!();

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
                top_list.push(arg);
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

    Parameter { file_list, defines, inc_list, top_list, rev, pkg }
}


fn show_info(p: &Parameter) {
    info!("package {}, rev {}", p.pkg, p.rev);
    if p.pkg == PKG_DEFAULT { warn!("package not set, use default '{}'", p.pkg) }
    if p.rev == REV_DEFAULT { warn!("revision not set, use default {}", p.rev) }
    if p.top_list.is_empty() { warn!("top list is empty") }

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


fn parse_files(p: &Parameter) -> Vec<SyntaxTree> {
    info!("parse files");

    let mut res: Vec<SyntaxTree> = vec![];

    let defines = to_defines(&p.defines);

    for file in p.file_list.iter() {
        info!("  parsing {} ...", file);

        let (syntax_tree, _) = parse_sv(&file, &defines, &p.inc_list, false, false).unwrap();

        res.push(syntax_tree)
    }

    res
}

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().skip(1).collect();
    let p = parse_args(args);

    show_info(&p);

    let _syntax_tree_list = parse_files(&p);
}
