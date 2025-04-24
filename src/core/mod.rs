#[macro_export]
macro_rules! osstr {
	[ $($a:tt)* ] => {
		OsStr::new( $($a)* )
	};
}
