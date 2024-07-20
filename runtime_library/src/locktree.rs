use core::panic;
use std::{cell::{Cell, Ref, RefCell, UnsafeCell}, collections::{HashSet, VecDeque}, hash::RandomState, vec::Vec};
//use std::cell::RefCell;
//use std::rc::{Rc, Weak};
use std::sync::{Arc, Weak, Mutex};
use crate::{memalloc::{GlobalSystemAllocatorType, GLOBAL_SYSTEM_ALLOCATOR}, solcon_report, utils::ThreadInfo};
use log::{debug, info, warn, error};
use hashbrown::{hash_map::DefaultHashBuilder, HashMap};

pub type LockId = u64;
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum LockType {
    MutexLock,
    ReadLock,
    WriteLock,
}

pub type HoldingLock = (LockId, LockType);
type LockTreeNodeRef = Arc<RefCell<ThreadLockTreeNode>, GlobalSystemAllocatorType>;
type LockTreeNodeWeakRef = Weak<RefCell<ThreadLockTreeNode>, GlobalSystemAllocatorType>;
type LockTreeNodeHashMap = HashMap< HoldingLock, LockTreeNodeRef, DefaultHashBuilder, allocator_api2::alloc::System>;
const TOPDUMMY_LOCK : HoldingLock = (0, LockType::MutexLock);
type OwnLocksSet = HashMap<HoldingLock, (Vec<LockTreeNodeRef>, usize), DefaultHashBuilder, allocator_api2::alloc::System>;

pub struct ThreadLockTree {
    thread: ThreadInfo,
    current: LockTreeNodeRef,
    root: LockTreeNodeRef,
    all_own_locks: OwnLocksSet,
}

fn format_own_locks_set(own_locks_set: &OwnLocksSet) -> String {
    let mut result = String::new();

    for (holding_lock, (vec_lock_tree_node_ref, usize_val)) in own_locks_set {
        result.push_str(&format!("{:?}: {} nodesaddr{:?} | ", holding_lock, usize_val, 
            vec_lock_tree_node_ref.iter().map(|arc| {
            Arc::as_ptr(arc)
            }).collect::<Vec<_>>()
        ));
    }
    result
}

// unsafe impl Sync for ThreadLockTree{}
unsafe impl Send for ThreadLockTree{}

pub struct ThreadLockTreeNode {
    locking_info: HoldingLock, 
    children: LockTreeNodeHashMap,
    parent: Option< LockTreeNodeWeakRef >,
}

impl ThreadLockTree {
    pub fn new(thread: ThreadInfo) -> Self {
        let root = Arc::new_in(RefCell::new(ThreadLockTreeNode{
            locking_info: TOPDUMMY_LOCK,
            children: HashMap::new_in(allocator_api2::alloc::System),
            parent: None,
        }), GLOBAL_SYSTEM_ALLOCATOR);
        ThreadLockTree{
            thread: thread,
            current: root.clone(),
            root: root,
            all_own_locks: HashMap::new_in(allocator_api2::alloc::System),
        }
    }
    pub fn record_lock(&mut self, locking_info: HoldingLock) {
        info!("locktree record_lock, {locking_info:?}");
        debug!("all_own_locks, {all_own_locks:?}", all_own_locks=format_own_locks_set(&self.all_own_locks));
        debug!("locktree, \n{locktree:?}", locktree=*self);
        // check rwlock double lock: get write when holding read or get read when holding write
        let waiting_lock_info = match locking_info.1 {
            LockType::WriteLock => self.all_own_locks.get(&(locking_info.0, LockType::ReadLock)),
            LockType::ReadLock => self.all_own_locks.get(&(locking_info.0, LockType::WriteLock)),
            LockType::MutexLock => None,
        };
        if let Some(waiting_lock_info) = waiting_lock_info {
            let waiting_lock = waiting_lock_info.0.last().unwrap().borrow();
            solcon_report!("found rwlock double lock on {lock:?} in thread {thread:?} , try to get {get_type:?} when holding already {hold_type:?}", 
                lock=locking_info.0, thread=self.thread,
                hold_type=waiting_lock.locking_info.1,
                get_type=locking_info.1
            );
        }
        let own_lock = self.all_own_locks.get(&locking_info);
        if let Some(own_lock) = own_lock {
            match locking_info.1 {
                LockType::MutexLock | LockType::WriteLock => {
                    solcon_report!("found double lock on {lock:?} in thread {thread:?} ", lock=locking_info, thread=self.thread);
                    //solcon_report!("all_own_locks, {all_own_locks:?}", all_own_locks=self.all_own_locks);
                }
                LockType::ReadLock => {
                    solcon_report!("found potential double read lock on lock {lock:?} in thread{thread:?}", lock=locking_info, thread=self.thread);
                    //solcon_report!("all_own_locks, {all_own_locks:?}", all_own_locks=self.all_own_locks);
                }
            };
            let own_lock = self.all_own_locks.get_mut(&locking_info).unwrap();
            let (nodes_ref, lock_cnt) = own_lock;
            *lock_cnt += 1;
        } else {
            self.all_own_locks.insert(locking_info, (vec![], 1));
        }
        debug!("all_own_locks, {all_own_locks:?}", all_own_locks=format_own_locks_set(&self.all_own_locks));
        let mut current = RefCell::borrow_mut(&self.current);
        if let Some(locktree_node) = current.children.get(&locking_info) {
            // update current
            let locktree_node_arc = Arc::clone(locktree_node);
            drop(current);
            self.current = Arc::clone(&locktree_node_arc);
            // update all_own_locks
            let own_lock = self.all_own_locks.get_mut(&locking_info).unwrap();
            own_lock.0.push(locktree_node_arc);
        } else {
            // create new ThreadLockTreeNode
            let locktree_node = ThreadLockTreeNode{
                locking_info: locking_info,
                children: HashMap::new_in(allocator_api2::alloc::System),
                parent: Some(Arc::downgrade(&self.current)),
            };
            let locktree_node_arc = Arc::new_in(RefCell::new(locktree_node), GLOBAL_SYSTEM_ALLOCATOR);
            current.children.insert(locking_info, Arc::clone(&locktree_node_arc));
            drop(current);
            info!("{}", locktree_node_arc.borrow().build_path());
            self.current = Arc::clone(&locktree_node_arc);
            // update all_own_locks
            let own_lock = self.all_own_locks.get_mut(&locking_info).unwrap();
            own_lock.0.push(locktree_node_arc);
        }
        info!("record_lock-after current-lockpath, {locking_info:?} {}", self.current.borrow().build_path());
    }
    
    pub fn record_unlock(&mut self, locking_info: HoldingLock) {
        info!("locktree record_unlock, {locking_info:?}");
        debug!("all_own_locks, {all_own_locks:?}", all_own_locks=format_own_locks_set(&self.all_own_locks));
        debug!("locktree, \n{locktree:?}", locktree=*self);
        let current = RefCell::borrow(&self.current);
        if current.locking_info == TOPDUMMY_LOCK {
            panic!("should not unlock when current is TOPDUMMY_LOCK")
        };
        let own_lock = self.all_own_locks.get(&locking_info);
        let Some((nodes_ref, lock_cnt)) = own_lock else {
            panic!("try to unlock a lock that not lock already");
        };
        match locking_info.1 {
            LockType::MutexLock | LockType::WriteLock => assert_eq!(*lock_cnt, 1),
            LockType::ReadLock => {},
        };
        let should_delete = *lock_cnt == 1;
        debug!("all_own_locks, {all_own_locks:?}", all_own_locks=format_own_locks_set(&self.all_own_locks));
        // update graph
        if current.locking_info != locking_info {
            drop(current);
            solcon_report!("warn: unlock in random order of lock instead of reverse order");
            // copy new lock path
            let unlocking_node = {
                let own_lock = self.all_own_locks.get(&locking_info);
                let (nodes_ref, lock_cnt) = own_lock.expect("some because previous check");
                let unlocking_node = nodes_ref.last().expect("at least exist here");
                Arc::clone(&unlocking_node)
            };
            let unlocking_node = RefCell::borrow(&unlocking_node) ;
            debug!("unlocking_node_path=");
            unlocking_node.build_path();
            debug!("unlocking_node.locking_info={:?}", unlocking_node.locking_info);
            let unlocking_node_parent = unlocking_node.parent.as_ref().expect("any node besides root has a parent");
            let copied_children_subtree = unlocking_node.children.iter().map(|child| {
                let (child_key, child_node) = child;
                info!("cloning {child_key:?}");
                let old_current = Arc::clone(&self.current);
                let new_subtree_root = ThreadLockTree::clone_subtree(&child_node, &old_current, &mut self.current, &mut self.all_own_locks);
                new_subtree_root.borrow_mut().parent = Some(Weak::clone(&unlocking_node_parent));
                (*child_key, new_subtree_root)
            });
            let unlocking_node_parent = unlocking_node_parent.upgrade().expect("parent cannot be free before childen");
            let mut unlocking_node_parent = RefCell::borrow_mut(&unlocking_node_parent);
            debug!("unlocking_node_parent.locking_info={:?}", unlocking_node_parent.locking_info);
            unlocking_node_parent.children.extend(copied_children_subtree);
            drop(unlocking_node_parent);
            debug!("all_own_locks rebuilt, {all_own_locks:?}", all_own_locks=format_own_locks_set(&self.all_own_locks));
            debug!("locktree added, \n{locktree:?}", locktree=*self);
        } else {
            // update current
            let current_parent = current.parent.as_ref().expect("any node that node.lock_id != TOP_LOCKID shoule has a parent");
            let new_current = current_parent.upgrade().expect("parent should not be freed before all children free");
            drop(current);
            self.current = new_current;
        }
        // update self.all_own_locks
        if should_delete {
            self.all_own_locks.remove(&locking_info);
        } else {
            let own_lock = self.all_own_locks.get_mut(&locking_info);
            let (nodes_ref, lock_cnt) = own_lock.expect("some because previous check");
            nodes_ref.pop();
            *lock_cnt -= 1;
        }   
        info!("record_unlock-after current-lockpath, {locking_info:?} {}", self.current.borrow().build_path());
    }

    fn exist_specific_lock_nodes(&self, locking_info: HoldingLock, gatelock_set: &HashSet<HoldingLock>) -> bool {
        assert!(!gatelock_set.contains(&locking_info));
        let mut queue = VecDeque::new_in(GLOBAL_SYSTEM_ALLOCATOR);
        queue.push_back(Arc::clone(&self.root));
        while let Some(x) = queue.pop_front() {
            let x_borrow = RefCell::borrow(&x);
            if x_borrow.locking_info == locking_info {
                return true;
            }
            for (_, child) in &x_borrow.children {
                if !gatelock_set.contains(&locking_info) {
                    queue.push_back(Arc::clone(&child));
                }
            }
        }
        false
    }

    fn collect_all_specific_lock_nodes(&self, locking_info: HoldingLock, gatelock_set: &HashSet<HoldingLock>) -> Vec<LockTreeNodeRef, GlobalSystemAllocatorType> {
        assert!(!gatelock_set.contains(&locking_info));
        let mut queue = VecDeque::new_in(GLOBAL_SYSTEM_ALLOCATOR);
        queue.push_back(Arc::clone(&self.root));
        let mut res = Vec::new_in(GLOBAL_SYSTEM_ALLOCATOR);
        while let Some(x) = queue.pop_front() {
            let x_borrow = RefCell::borrow(&x);
            if x_borrow.locking_info == locking_info {
                res.push(Arc::clone(&x));
            }
            for (_, child) in &x_borrow.children {
                if !gatelock_set.contains(&locking_info) {
                    queue.push_back(Arc::clone(&child));
                }
            }
        }
        res
    }

    fn clone_subtree(subtree_root: &LockTreeNodeRef, 
        old_current: &LockTreeNodeRef, new_current: &mut LockTreeNodeRef,
        all_own_locks: &mut OwnLocksSet,
    ) -> LockTreeNodeRef {
        let set_new_current = Arc::ptr_eq(old_current, &subtree_root);
        let subtree_root_borrow = subtree_root.borrow();
        let clone_self: ThreadLockTreeNode = ThreadLockTreeNode {
            locking_info: subtree_root_borrow.locking_info.clone(), 
            children: HashMap::new_in(allocator_api2::alloc::System),
            parent: subtree_root_borrow.parent.clone(),
        };
        let new_subroot = Arc::new_in(RefCell::new(clone_self), GLOBAL_SYSTEM_ALLOCATOR);
        new_subroot.borrow_mut().children = subtree_root.borrow().children.iter().map(|child| {
            let (child_key, child_node) = child;
            let newchild = Self::clone_subtree(&child_node, old_current, new_current, all_own_locks);
            newchild.borrow_mut().parent = Some(Arc::downgrade(&new_subroot));
            (*child_key, newchild)
        }).collect();
        if set_new_current {
            *new_current = Arc::clone(&new_subroot);
        }
        let t = all_own_locks.get_mut(&subtree_root_borrow.locking_info);
        if let Some(t) = t {
            for tt in &mut t.0 {
                if Arc::ptr_eq(tt, &subtree_root) {
                    *tt = Arc::clone(&new_subroot);
                }
            }
        }
        new_subroot
    }
}

impl std::fmt::Debug for ThreadLockTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut leaf_nodes = Vec::new_in(GLOBAL_SYSTEM_ALLOCATOR);
        ThreadLockTreeNode::collect_leaf_nodes(&self.root, &mut leaf_nodes);
        for leaf in leaf_nodes {
            let b = leaf.borrow();
            let path = b.build_path();
            writeln!(f, "{path}")?
        }
        writeln!(f)
    }
}

impl ThreadLockTreeNode {
    pub fn gatelock_marked_or_has_gatelock_marked_parent(&self, gatelock_set: &HashSet<HoldingLock>) -> bool {
        if gatelock_set.contains(&self.locking_info) {
            return true;
        }
        if let Some(parent_weak) = &self.parent {
            let parent = parent_weak.upgrade().expect("parent should not be freed before all children free");
            if RefCell::borrow(&parent).gatelock_marked_or_has_gatelock_marked_parent(gatelock_set) 
            {
                return true;
            }
        }
        false
    }

    fn _build_path(&self, s: &mut String) {
        if let Some(parent_weak) = &self.parent {
            let parent = parent_weak.upgrade().expect("parent should not be freed before all children free");
            RefCell::borrow(&parent)._build_path(s);
            s.push_str(format!("->{:?}", self.locking_info).as_str());
        }
    }

    fn build_path(&self) -> String {
        let mut path_str: String = String::from("");
        self._build_path(&mut path_str);
        path_str
    }

    fn collect_leaf_nodes(subtree_root: &LockTreeNodeRef, result: &mut Vec<LockTreeNodeRef, GlobalSystemAllocatorType> ) {
        if subtree_root.borrow().children.is_empty() {
            result.push(Arc::clone(&subtree_root));
        }
        for (_, child) in &subtree_root.borrow().children {
            Self::collect_leaf_nodes(child, result);
        }
    }

    fn merge_subtree_to_children(&mut self) {
        unimplemented!("should implement and used after clone subtree")
    }
}

impl PartialEq for ThreadLockTreeNode {
    fn eq(&self, rhs: &ThreadLockTreeNode) -> bool {
        return self.locking_info.0 == rhs.locking_info.0 && self.locking_info.1 == rhs.locking_info.1; 
    }
}

pub fn analyze_potential_conflict_locks(locktree1: &ThreadLockTree, locktree2: &ThreadLockTree) {
    let mut gatelock_set = HashSet::new();
    let t1topnodes = &RefCell::borrow(&locktree1.root).children;
    for (_, n) in t1topnodes {
        assert!(gatelock_set.is_empty());
        analyze_potential_conflict_locks_for_lockpath(locktree1, locktree2, &n, &mut gatelock_set);
        assert!(gatelock_set.is_empty());
    }
}

// 递归遍历t1中的节点n1，尝试在n2中找到相同锁的节点，然后查找冲突锁定（n1向下查找子，n2向上查找父）
// n 代表 t1中某个lockpath的中的某个节点n
pub fn analyze_potential_conflict_locks_for_lockpath(t1: &ThreadLockTree, t2: &ThreadLockTree, n1: &LockTreeNodeRef, gatelock_set: &mut HashSet<HoldingLock>) {
    let n1_borrow = RefCell::borrow(&n1);
    let s = t2.collect_all_specific_lock_nodes(n1_borrow.locking_info, gatelock_set);
    for n2 in &s {
        assert_eq!(n1_borrow.locking_info, n2.borrow().locking_info);
        // n1向下查找子，n2向上查找父
        check(t1, t2, &n1, &n2, gatelock_set);
        // 当前结点及其所有父结点对应的锁，都不可能被t2同时获取。
        // 因此t2中的所有相同锁的结点的子树都不需要考虑（即使其中有不同的锁定顺序，也被gatelock——也即n1.lock_id==n2.lock_id——保护了）。
        // t1当前结点的父结点对应的锁在t2中的相同锁的结点在上层递归中已经被mark
        // mark t1当前结点对应的锁的t2相同锁的结点，以不检测t2中相同锁的节点的子树。
        match n2.borrow().locking_info.1 {
            LockType::MutexLock | LockType::WriteLock => {
                gatelock_set.insert(n2.borrow().locking_info);
            },
            LockType::ReadLock => {}, // ReadLock cannot become a gatelock
        }
    }
    for (_, nchild) in &n1_borrow.children {
        analyze_potential_conflict_locks_for_lockpath(t1, t2, &nchild, gatelock_set);
    }
    // unmark。马上要返回检测其他路径了。
    for n2 in s {
        let n2 = RefCell::borrow(&n2);
        gatelock_set.remove(&n2.locking_info);
    }
}

// n1向下查找子，n2向上查找父
// n1借助递归向下遍历孩子，n2在每次递归中保持不变
pub fn check(t1: &ThreadLockTree, t2: &ThreadLockTree, n1: &LockTreeNodeRef, n2: &LockTreeNodeRef, gatelock_set: &HashSet<HoldingLock>) -> bool {
    let n1_borrow = n1.borrow();
    for (_, n1gchild) in &n1_borrow.children {
        if _check(t1, t2, n1, n2, n1gchild, gatelock_set) {
            return true;
        }
    }
    false
}

// n1向下查找子，n2向上查找父
// n1借助递归向下遍历孩子，n2在每次递归中保持不变
pub fn _check(t1: &ThreadLockTree, t2: &ThreadLockTree, n1: &LockTreeNodeRef, n2: &LockTreeNodeRef, n1_gchild: &LockTreeNodeRef, gatelock_set: &HashSet<HoldingLock>) -> bool {
    let n1_borrow = n1.borrow();
    let n1gchild_borrow = n1_gchild.borrow();
    match (n1_borrow.locking_info.1, n1gchild_borrow.locking_info.1) {
        (LockType::MutexLock, LockType::MutexLock)  if n1_borrow.locking_info.0 == n1gchild_borrow.locking_info.0 => {
            unreachable!("should detect already as double lock")
        }
        (LockType::WriteLock, LockType::WriteLock)  if n1_borrow.locking_info.0 == n1gchild_borrow.locking_info.0 => {
            unreachable!("should detect already as double lock")
        }
        (LockType::ReadLock, LockType::ReadLock) if n1.borrow().locking_info.0 == n1gchild_borrow.locking_info.0 => {
            let exist_cocurrent_writer = t2.exist_specific_lock_nodes((n1.borrow().locking_info.0, LockType::WriteLock), gatelock_set);
            solcon_report!("double read lock(RR), {exist_cocurrent_writer}");
            // 如果只有读者线程，不会死锁。但如果引入同一RwLock上申请WriteLock的线程
            // t1 t2
            // r    
            //    w
            // r
        }
        (LockType::MutexLock, LockType::MutexLock) 
        | (LockType::WriteLock, LockType::WriteLock)
        | (LockType::MutexLock, LockType::WriteLock)
        | (LockType::WriteLock, LockType::MutexLock)
        => {
            if let Some(n2_locked_parent) = own_lock_in_parent(n2, n1gchild_borrow.locking_info) { // n1child.lock 在 n2 上面
               // conflict pair is 
                // t1: n1.lock + n1gchild.lock
                // t2: n1gchild.lock + n2.lock(n1.lock==n2.lock)
                solcon_report!("t1-first: {}", n1_borrow.build_path());
                solcon_report!("t1-second: {}", n1gchild_borrow.build_path());
                solcon_report!("t2-first: {}", n2_locked_parent.borrow().build_path());
                solcon_report!("t2-second: {}", n2.borrow().build_path());
                solcon_report!("conflict lock found, {:?}", (n1_borrow.locking_info.1, n1gchild_borrow.locking_info.1));
 
            }
        },
        (LockType::MutexLock, LockType::ReadLock) | (LockType::ReadLock, LockType::MutexLock) 
        | (LockType::ReadLock, LockType::WriteLock) | (LockType::WriteLock, LockType::ReadLock) 
        => {
            if let Some(n2_locked_parent) = own_lock_in_parent(n2, n1gchild_borrow.locking_info) { // n1child.lock 在 n2 上面
                solcon_report!("t1-first: {}", n1_borrow.build_path());
                solcon_report!("t1-second: {}", n1gchild_borrow.build_path());
                solcon_report!("t2-first: ");
                solcon_report!("t2-first: {}", n2_locked_parent.borrow().build_path());
                solcon_report!("t2-second: {}", n2.borrow().build_path());
                solcon_report!("potential conflict read lock(RM/MR/RW/WR) {:?}", (n1.borrow().locking_info.1, n1gchild_borrow.locking_info.1));
                // 如果只有两个线程，不会死锁。但如果引入同一RwLock上申请WriteLock的第三线程
                // 则两个申请ReadLock的线程可借中间的WriteLock申请建立冲突关系
                // t1 t3 t2
                // m      r
                //    w   
                //        m
                // r
                // todo!("check writer really exist in t3");
            }
        },
        (LockType::ReadLock, LockType::ReadLock) => {
            /*
                t1               t2
                            write RW2
                read RW1
                            write RW1(block here)
                read RW2(block here)
            */
            // check write RW2 & write RW1 really exist in t2
            let exist_cocurrent_writer1 = t2.exist_specific_lock_nodes((n1.borrow().locking_info.0, LockType::WriteLock), gatelock_set);
            let exist_cocurrent_writer2 = t2.exist_specific_lock_nodes((n1gchild_borrow.locking_info.0, LockType::WriteLock), gatelock_set);
            if exist_cocurrent_writer1 && exist_cocurrent_writer2 {
                solcon_report!("conflict read lock(RR)");
            } else {
                solcon_report!("potential conflict read lock(RR), {exist_cocurrent_writer1} {exist_cocurrent_writer2}");
            }
        }
    }
    for (_, n1child) in &n1gchild_borrow.children {
        _check(t1, t2, n1, n2, n1child, gatelock_set);
    }
    false
}

pub fn own_lock_in_parent(node: &LockTreeNodeRef, locking_info: HoldingLock) -> Option<LockTreeNodeRef> {
    let s = node.borrow();
    if s.locking_info == locking_info {
        return Some(Arc::clone(&node));
    }
    if let Some(parent_weak) = &s.parent {
        let parent = parent_weak.upgrade().expect("parent should not be freed before all children free");
        return own_lock_in_parent(&parent, locking_info);
    }
    None
}
