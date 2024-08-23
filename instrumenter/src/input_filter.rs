 use std::borrow::Borrow;
 use std::collections::HashSet;

use rustc_middle::ty::{self, TyCtxt};
use rustc_span::{
    def_id::{CrateNum, DefId, DefIndex, LOCAL_CRATE},
    DUMMY_SP,
};
use tracing::info;
use lazy_static::lazy_static;

enum Filter {
    All,
    WhiteList(HashSet<String>),
    Regexp(regex::Regex),
}

lazy_static! {
    static ref FILTER: Filter = {
        // 检查环境变量
        if let Some(filter_value) = std::env::var("SOLCON_INPUT_FILTER").ok() {
            if filter_value.starts_with("[") && filter_value.ends_with("]")  {
                // 创建一个 HashSet 来存储分割后的值
                let mut set = HashSet::new();
                // 使用逗号分割字符串，并添加到 HashSet 中
                for value in filter_value.split(',') {
                    set.insert(value.to_string());
                }
                info!("SOLCON_INPUT_FILTER WhiteList: {}", filter_value);
                Filter::WhiteList(set)
            } else {
                let regex = regex::Regex::new(&filter_value);
                if let Some(regex) = regex.ok() {
                    info!("SOLCON_INPUT_FILTER Regex: {}", filter_value);
                    Filter::Regexp(regex)
                } else {
                    info!("Invalid SOLCON_INPUT_FILTER: {}", filter_value);
                    Filter::All
                }
            }
        } else {
            Filter::All
        }
    };
}

pub fn should_process<'tcx>(tcx: TyCtxt<'tcx>) -> bool {
    match &*FILTER {
        Filter::All => true,
        Filter::WhiteList(set) => {
            let crate_name = tcx.crate_name(LOCAL_CRATE);
            set.contains(crate_name.as_str())
        }
        Filter::Regexp(regexes) => {
            let crate_name = tcx.crate_name(LOCAL_CRATE);
            regexes.is_match(crate_name.as_str())
        }
    }
}

fn is_target_crate(tcx: TyCtxt<'_>, krate: &CrateNum, target_crates: &[&str]) -> bool {
    let crate_name = tcx.crate_name(*krate);
    let crate_name_str = crate_name.as_str();
    target_crates.contains(&crate_name_str)
}