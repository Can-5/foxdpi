.PHONY: dev lint test wheel clean bypass-rust bypass-python install-dev

dev:
	pip install --quiet --break-system-packages -e .
	python -m foxdpi doctor

lint:
	ruff check foxdpi
	ruff format --check foxdpi

test:
	pytest -q

wheel:
	python -m build --wheel

clean:
	rm -rf build dist *.egg-info
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true

# foxdpi-bypass Rust binary'si (şablon)
bypass-rust:
	cd bypass && cargo build --release
	cp bypass/target/release/foxdpi-bypass foxdpi-bypass

# foxdpi-bypass Python paketi (şablon)
bypass-python:
	cd python_packages/foxdpi-bypass && pip install -e .

# install.sh test (şablon)
install-dev:
	FORDPI_PREFIX=$HOME/.local ./install.sh --prefix $HOME/.local-test
