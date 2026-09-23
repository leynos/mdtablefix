// Shared Cargo and Verus expansion forms for the production scanner.

/// Emits the ordinary Rust form of a kernel function for Cargo builds.
#[cfg(not(verus_keep_ghost))]
macro_rules! verified_kernel_function {
    (
        $(#[doc = $doc:expr])*
        $(#[must_use])?
        $(#[cfg_attr(verus_keep_ghost, verifier::$external:ident)])?
        $visibility:vis fn $name:ident($($arguments:tt)*) -> $result:ty;
        $(requires($($precondition:tt)*);)?
        ensures($result_name:ident => $($postcondition:tt)*);
        $body:block
    ) => {
        $(#[doc = $doc])*
        #[must_use]
        $(#[cfg_attr(verus_keep_ghost, verifier::$external)])?
        $visibility fn $name($($arguments)*) -> $result $body
    };
}

/// Emits a scanner loop from one body while omitting proof annotations in Cargo.
#[cfg(not(verus_keep_ghost))]
macro_rules! verified_loop_function {
    (
        $(#[doc = $doc:expr])*
        $(#[must_use])?
        $(#[cfg_attr(verus_keep_ghost, verifier::$external:ident)])?
        $visibility:vis fn $name:ident($($arguments:tt)*) -> $result:ty;
        $(requires($($precondition:tt)*);)?
        ensures($result_name:ident => $($postcondition:tt)*);
        before { $($before:tt)* }
        while ($condition:expr) invariant($($invariant:tt)*) $loop_body:block
        $(proof_after { $($proof_after:tt)* })?
        after { $($after:tt)* }
    ) => {
        $(#[doc = $doc])*
        #[must_use]
        $(#[cfg_attr(verus_keep_ghost, verifier::$external)])?
        $visibility fn $name($($arguments)*) -> $result {
            $($before)*
            while $condition $loop_body
            $($after)*
        }
    };
}

/// Sends the same scanner loop and its invariant to Verus.
#[cfg(verus_keep_ghost)]
macro_rules! verified_loop_function {
    (
        $(#[doc = $doc:expr])*
        $(#[must_use])?
        $(#[cfg_attr(verus_keep_ghost, verifier::$external:ident)])?
        $visibility:vis fn $name:ident($($arguments:tt)*) -> $result:ty;
        $(requires($($precondition:tt)*);)?
        ensures($result_name:ident => $($postcondition:tt)*);
        before { $($before:tt)* }
        while ($condition:expr) invariant($($invariant:tt)*) $loop_body:block
        $(proof_after { $($proof_after:tt)* })?
        after { $($after:tt)* }
    ) => {
        verus! {
            $(#[doc = $doc])*
            #[must_use]
            $(#[cfg_attr(verus_keep_ghost, verifier::$external)])?
            $visibility fn $name($($arguments)*) -> ($result_name: $result)
                $(requires $($precondition)*)?
                ensures $($postcondition)*
            {
                $($before)*
                while $condition
                    invariant $($invariant)*
                    $loop_body
                $(proof { $($proof_after)* })?
                $($after)*
            }
        }
    };
}

/// Emits the contracted Verus form of the same kernel function body.
#[cfg(verus_keep_ghost)]
macro_rules! verified_kernel_function {
    (
        $(#[doc = $doc:expr])*
        $(#[must_use])?
        $(#[cfg_attr(verus_keep_ghost, verifier::$external:ident)])?
        $visibility:vis fn $name:ident($($arguments:tt)*) -> $result:ty;
        $(requires($($precondition:tt)*);)?
        ensures($result_name:ident => $($postcondition:tt)*);
        $body:block
    ) => {
        verus! {
            $(#[doc = $doc])*
            #[must_use]
            $(#[cfg_attr(verus_keep_ghost, verifier::$external)])?
            $visibility fn $name($($arguments)*) -> ($result_name: $result)
                $(requires $($precondition)*)?
                ensures $($postcondition)*
                $body
        }
    };
}
