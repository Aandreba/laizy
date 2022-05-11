check:
	cargo check
	cargo check --features futures
	rustup run nightly cargo check --all-features

doc:
	rustup run nightly cargo rustdoc --open --all-features -- --cfg docsrs

publish: check
	cargo publish