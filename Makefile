check:
	cargo check
	cargo check --all-features

doc:
	rustup run nightly cargo rustdoc --open --all-features -- --cfg docsrs