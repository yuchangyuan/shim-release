use std::{env};
use log::{debug, info, log_enabled, warn, Level};
use std::collections::{BTreeMap};
use env_logger::Env;

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

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().skip(1).collect();
    let p = parse_args(args);

    show_info(&p)
}
