 use std::borrow::Borrow;
 use std::collections::HashSet;

use rustc_middle::ty::{self, TyCtxt};
use rustc_span::{
    def_id::{CrateNum, DefId, DefIndex, LOCAL_CRATE},
    DUMMY_SP,
};
use tracing::info;
use lazy_static::lazy_static;

lazy_static! {
    static ref FILTER_SET: HashSet<String> = {
        // 检查环境变量
        if let Some(filter_value) = std::env::var("SOLCON_INPUT_FILTER").ok() {
            // 创建一个 HashSet 来存储分割后的值
            let mut set = HashSet::new();
            // 使用逗号分割字符串，并添加到 HashSet 中
            for value in filter_value.split(',') {
                set.insert(value.to_string());
            }
            set
        } else {
            HashSet::new()
        }
    };
}

pub fn should_process<'tcx>(tcx: TyCtxt<'tcx>) -> bool {
    if FILTER_SET.is_empty() {
        return true;
    }
    let crate_name = tcx.crate_name(LOCAL_CRATE).as_str().to_owned();
    FILTER_SET.contains(&crate_name)
}

fn is_target_crate(tcx: TyCtxt<'_>, krate: &CrateNum, target_crates: &[&str]) -> bool {
    let crate_name = tcx.crate_name(*krate);
    let crate_name_str = crate_name.as_str();
    target_crates.contains(&crate_name_str)
}