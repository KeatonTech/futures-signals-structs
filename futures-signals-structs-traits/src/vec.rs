use crate::{AsMutableStruct, MutableStruct};
use futures_signals::signal_vec::MutableVec;

impl<T: Clone> AsMutableStruct for Vec<T> {
    type MutableStructType = MutableVec<T>;

    fn as_mutable_struct(&self) -> Self::MutableStructType {
        MutableVec::new_with_values(self.clone())
    }
}

impl<T: Clone> MutableStruct for MutableVec<T> {
    type SnapshotType = Vec<T>;

    fn snapshot(&self) -> Self::SnapshotType {
        self.lock_ref().as_slice().to_vec()
    }

    fn update(&self, new_snapshot: Self::SnapshotType) {
        self.lock_mut().replace_cloned(new_snapshot);
    }
}