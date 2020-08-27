pub trait MutableStruct {
    type SnapshotType;

    /// Returns a non-mutable version of this struct, which is a basic Rust struct
    /// that can be passed around to code that is not aware of futures-signals.
    /// 
    /// Note that 'non-mutable' in this context does not mean immutable in the Rust
    /// sense. It just means that the struct is not a MutableStruct and therefore
    /// changes are not tracked by futures-signals.
    fn snapshot(&self) -> Self::SnapshotType;

    /// Updates every field in this MutableStruct to match an non-mutable struct.
    fn update(&self, new_snapshot: Self::SnapshotType);
}

pub trait AsMutableStruct {
    type MutableStructType: MutableStruct;

    /// Returns a Mutable version of this struct. Note that mutable in this context
    /// does not mean `mut` in the Rust sense. Instead it means that every field on
    /// the returned struct will be an instance of `Mutable` as provided in the
    /// futures-signals crate. This means that any changes to the struct can be
    /// tracked using signals.
    fn as_mutable_struct(&self) -> Self::MutableStructType;
}