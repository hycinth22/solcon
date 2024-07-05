use std::collections::BTreeMap;

use rustc_hash::FxHashMap;
use rustc_middle::{bug, span_bug};
use rustc_middle::ty::{Ty, TyCtxt, TyKind};
use rustc_middle::mir::{BasicBlock, BasicBlockData, Body, CallSource, Local, Location, Operand, Place, Rvalue, SourceInfo, StatementKind, Terminator, TerminatorKind, UnwindAction, UnwindTerminateReason};
use rustc_middle::mir::visit::{PlaceContext, Visitor, NonMutatingUseContext, MutatingUseContext};
use rustc_middle::mir::patch::MirPatch;
use rustc_span::source_map::Spanned;
use rustc_span::{Span, DUMMY_SP};

use crate::monitors_finder::MonitorsInfo;
use crate::utils;

pub fn instrument_mem_acesses<'tcx>(tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>, 
monitors: &MonitorsInfo) {
    let Some(mem_read_before_fn_defid) = monitors.mem_read_before_fn else {
        return;
    };
    let Some(mem_write_before_fn_defid) = monitors.mem_write_before_fn else {
        return;
    };
    info!("instrumenting memory acesses");

    let deref_pointers_locations = {
        let mut c = PlaceDerefCollector::new(tcx);
        c.visit_body(body);
        // sort to reversed order for each basic block
        let mut deref_pointers = c.deref_pointers.into_iter().
            map(|(bb, op_map)| {
                (   
                    bb,
                    op_map.into_iter().map(|
                        (loc_index, (r, w))|  (loc_index, r, w)
                    ).collect::<Vec<_>>()
                )
            }).collect::<Vec<_>>();
        for (_, deref_pointers) in &mut deref_pointers {
            deref_pointers.sort_by(|a, b| b.0.cmp(&a.0));
        }
        deref_pointers
    };

    let mut patch = MirPatch::new(body);
    let useless_temp = patch.new_temp(tcx.types.unit, DUMMY_SP);
    for (block, deref_pointers) in deref_pointers_locations {
        //continue;
        let bb_data = &mut body.basic_blocks_mut()[block];
        let bb_terminator = &mut bb_data.terminator;
        let bb_statements = &mut bb_data.statements;
        let bb_is_cleanup = bb_data.is_cleanup;
        for (statement_index, read_local, write_local) in deref_pointers {
            let span = bb_statements[statement_index].source_info.span;
            let statement_source_info = bb_statements[statement_index].source_info;
            // let mut new_bb_statements = Vec::new();
            // bb_statements[statement_index..].iter_mut().for_each(|s| {
            //         new_bb_statements.push(s.clone());
            //         s.make_nop();
            //     }
            // );
            info!("split_off {block:?} {statement_index:?}");
            if let Some(read_local) = read_local {
                let new_bb_statements = bb_statements.split_off(statement_index);
                info!("instrumenting read {:?} {:?} {:?}", block, statement_index, read_local);
                let read_raw_pointer = patch.new_temp(tcx.types.usize, span);
                patch.add_statement(Location{block, statement_index}, StatementKind::Assign(
                    Box::new(
                        (
                            Place::from(read_raw_pointer),
                            Rvalue::AddressOf(
                                rustc_middle::ty::Mutability::Not,
                                Place {
                                    local: read_local,
                                    projection: tcx.mk_place_elems(&[
                                        rustc_middle::mir::ProjectionElem::Deref
                                    ]),
                                },
                           ),
                        )
                    )
                ));
                let read_addr = patch.new_temp(tcx.types.usize, span);
                patch.add_statement(Location{block, statement_index}, StatementKind::Assign(
                    Box::new(
                        (
                            Place::from(read_addr),
                            Rvalue::Cast(
                                rustc_middle::mir::CastKind::PointerExposeProvenance,
                                Operand::Move(Place::from(read_raw_pointer).into()),
                                tcx.types.usize
                            ),
                        )
                    )
                ));
                let monitor_args = vec![
                    Spanned{
                        span: span,
                        node: Operand::Copy(Place::from(read_addr).into()),
                    }
                ];
                let newbb = patch.new_block(BasicBlockData {
                    statements: new_bb_statements,
                    terminator: bb_terminator.clone(),
                    is_cleanup: bb_is_cleanup,
                });
                *bb_terminator = Some(Terminator{
                    source_info: statement_source_info,
                    // kind: TerminatorKind::Goto { target: newbb },
                    kind: TerminatorKind::Call {
                        func: utils::instantiate_our_func(tcx, mem_read_before_fn_defid, [], DUMMY_SP),
                        args: monitor_args,
                        destination: useless_temp.into(),
                        target: Some(newbb),
                        unwind: UnwindAction::Terminate(UnwindTerminateReason::Abi),
                        call_source: CallSource::Misc,
                        fn_span: span,
                    }
                });
            }
            if let Some(write_local) = write_local {
                let new_bb_statements = bb_statements.split_off(statement_index);
                info!("instrumenting write {:?} {:?} {:?}", block, statement_index, write_local);
                let write_raw_pointer = patch.new_temp(tcx.types.usize, span);
                patch.add_statement(Location{block, statement_index}, StatementKind::Assign(
                    Box::new(
                        (
                            Place::from(write_raw_pointer),
                            Rvalue::AddressOf(
                                rustc_middle::ty::Mutability::Not,
                                Place {
                                    local: write_local,
                                    projection: tcx.mk_place_elems(&[
                                        rustc_middle::mir::ProjectionElem::Deref
                                    ]),
                                },
                           ),
                        )
                    )
                ));
                let write_addr = patch.new_temp(tcx.types.usize, span);
                patch.add_statement(Location{block, statement_index}, StatementKind::Assign(
                    Box::new(
                        (
                            Place::from(write_addr),
                            Rvalue::Cast(
                                rustc_middle::mir::CastKind::PointerExposeProvenance,
                                Operand::Move(Place::from(write_raw_pointer).into()),
                                tcx.types.usize
                            ),
                        )
                    )
                ));
                let monitor_args = vec![
                    Spanned{
                        span: span,
                        node: Operand::Copy(Place::from(write_addr).into()),
                    }
                ];
                let newbb = patch.new_block(BasicBlockData {
                    statements: new_bb_statements,
                    terminator: bb_terminator.clone(),
                    is_cleanup: bb_is_cleanup,
                });
                *bb_terminator = Some(Terminator{
                    source_info: statement_source_info,
                    // kind: TerminatorKind::Goto { target: newbb },
                    kind: TerminatorKind::Call {
                        func: utils::instantiate_our_func(tcx, mem_write_before_fn_defid, [], DUMMY_SP),
                        args: monitor_args,
                        destination: useless_temp.into(),
                        target: Some(newbb),
                        unwind: UnwindAction::Terminate(UnwindTerminateReason::Abi),
                        call_source: CallSource::Misc,
                        fn_span: span,
                    }
                });
            }
        }
    }
    patch.apply(body);
    
}

type PointerDerefed = Local;
type BasicBlockOfPointerDeref = BasicBlock;
type StatementIndexOfPointerDeref = usize;
struct PlaceDerefCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    deref_pointers: FxHashMap<
        BasicBlockOfPointerDeref,
        FxHashMap<
            StatementIndexOfPointerDeref,
            (
                Option<PointerDerefed>,
                Option<PointerDerefed> 
            ) 
        >
    >,
}

impl<'tcx> PlaceDerefCollector<'tcx> {
    fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            deref_pointers: FxHashMap::default(),
        }
    }
    fn collect_read(&mut self, block:BasicBlock, statement_index:usize, read_local:Local) {
        let deref_pointers = self.deref_pointers.entry(block).or_insert(FxHashMap::default());
        let entry = deref_pointers.entry(statement_index).or_insert((None, None));
        if entry.0.is_none() {
            entry.0 = Some(read_local);
        } else {
            bug!("currrently collect only one read on a statement");
        }
    }
    fn collect_write(&mut self, block:BasicBlock, statement_index:usize, write_local:Local) {
        let deref_pointers = self.deref_pointers.entry(block).or_insert(FxHashMap::default());
        let entry = deref_pointers.entry(statement_index).or_insert((None, None));
        if entry.1.is_none() {
            entry.1 = Some(write_local);
        } else {
            bug!("currrently collect only one write on a statement");
        }
    } 
}

impl<'tcx> Visitor<'tcx> for PlaceDerefCollector<'tcx> {
    fn visit_place(&mut self, place:&Place<'tcx>, context:PlaceContext, location:Location) {
        // 1. check deref exists
        // for MIR phases AnalysisPhase::PostCleanup and later, 
        // Deref projections can only occur as the first projection. 
        if !place.is_indirect_first_projection() {
            return;
        }
        // 2. check use context
        match context {
            PlaceContext::NonMutatingUse(u) => match u {
                NonMutatingUseContext::Inspect 
                | NonMutatingUseContext::Copy 
                | NonMutatingUseContext::Move 
                => {
                    let pointer = place.local;
                    info!("collected read {:?} {:?}", location, place);
                    self.collect_read(location.block, location.statement_index, pointer);
                },
                NonMutatingUseContext::SharedBorrow | NonMutatingUseContext::AddressOf => {},
                NonMutatingUseContext::PlaceMention | NonMutatingUseContext::Projection => {},
                NonMutatingUseContext::FakeBorrow => unreachable!("FakeBorrow is disallowed here")
            },
            PlaceContext::MutatingUse(u) => match u {
                MutatingUseContext::Store
                | MutatingUseContext::Call | MutatingUseContext::Yield
                | MutatingUseContext::AsmOutput
                | MutatingUseContext::SetDiscriminant
                | MutatingUseContext::Drop | MutatingUseContext::Deinit => {
                    let pointer = place.local;
                    info!("collected write {:?} {:?}", location, place);
                    self.collect_write(location.block, location.statement_index, pointer);
                },
                MutatingUseContext::Borrow | MutatingUseContext::AddressOf => {}
                MutatingUseContext::Projection| MutatingUseContext::Retag => {},
            },
            PlaceContext::NonUse(_) => {},
        }
    }
}

struct PlaceRWACollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    mem_acesses: BTreeMap<Location, (Vec<Place<'tcx>>, Vec<Place<'tcx>>, Vec<Place<'tcx>> ) >,
}

impl<'tcx> PlaceRWACollector<'tcx> {
    fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            mem_acesses: BTreeMap::default(),
        }
    }
}

impl<'tcx> Visitor<'tcx> for PlaceRWACollector<'tcx> {
    fn visit_place(&mut self, place:&Place<'tcx>, context:PlaceContext, location:Location) {
        match context {
            PlaceContext::NonMutatingUse(u) => match u {
                NonMutatingUseContext::Inspect 
                | NonMutatingUseContext::Copy 
                | NonMutatingUseContext::Move 
                => {
                    // debug!("read {u:?} {:?}", place);
                    if let Some(v) = self.mem_acesses.get_mut(&location) {
                        v.0.push(*place);
                    } else {
                        self.mem_acesses.insert(location, (vec![*place], Vec::new(), Vec::new()));
                    }
                },
                NonMutatingUseContext::SharedBorrow
                | NonMutatingUseContext::AddressOf => {
                    if let Some(v) = self.mem_acesses.get_mut(&location) {
                        v.2.push(*place);
                    } else {
                        self.mem_acesses.insert(location, (Vec::new(), Vec::new(), vec![*place]));
                    }
                },
                NonMutatingUseContext::PlaceMention
                | NonMutatingUseContext::Projection => {},
                NonMutatingUseContext::FakeBorrow => unreachable!("FakeBorrow is disallowed here")
            },
            PlaceContext::MutatingUse(u) => match u {
                MutatingUseContext::Store
                | MutatingUseContext::Call | MutatingUseContext::Yield
                | MutatingUseContext::AsmOutput
                | MutatingUseContext::SetDiscriminant
                | MutatingUseContext::Drop | MutatingUseContext::Deinit => {
                    // debug!("store {u:?} {:?}", place);
                    if let Some(v) = self.mem_acesses.get_mut(&location) {
                        v.1.push(*place);
                    } else {
                        self.mem_acesses.insert(location, (Vec::new(), vec![*place], Vec::new()));
                    }
                },
                MutatingUseContext::Borrow
                | MutatingUseContext::AddressOf => {
                    if let Some(v) = self.mem_acesses.get_mut(&location) {
                        v.2.push(*place);
                    } else {
                        self.mem_acesses.insert(location, (Vec::new(), Vec::new(), vec![*place]));
                    }
                }
                | MutatingUseContext::Projection
                | MutatingUseContext::Retag => {},
            },
            PlaceContext::NonUse(_) => {},
        }
        self.super_place(place, context, location);
    }
}
