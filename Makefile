.PHONY: dev lint test wheel clean

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
