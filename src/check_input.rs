use rustc_middle::ty::{self, TyCtxt};
use rustc_span::def_id::CrateNum;

pub fn should_process<'tcx>(tcx: TyCtxt<'tcx>) -> bool {
    // TODO: implement an filter here
    return true;
}

fn is_target_crate(tcx: TyCtxt<'_>, krate: &CrateNum, target_crates: &[&str]) -> bool {
    let crate_name = tcx.crate_name(*krate);
    let crate_name_str = crate_name.as_str();
    target_crates.contains(&crate_name_str)
}