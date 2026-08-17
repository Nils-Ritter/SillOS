.PHONY: all build run test

all: build

build:
	cargo build

run:
	cargo run

test:
	@set +e; \
	output=$$(cargo test --no-fail-fast 2>&1); \
	status=$$?; \
	echo "$$output"; \
	passed=$$(echo "$$output" | grep -c '\[TEST_OK\]'); \
	failed=$$(echo "$$output" | grep -c '\[TEST_FAIL\]'); \
	total=$$((passed + failed)); \
	if [ "$$total" -gt 0 ]; then \
		percent=$$((passed * 100 / total)); \
	else \
		percent=0; \
	fi; \
	bar_width=30; \
	if [ "$$total" -gt 0 ]; then \
		filled=$$((passed * bar_width / total)); \
	else \
		filled=0; \
	fi; \
	empty=$$((bar_width - filled)); \
	cyan=$$(printf '\033[0;36m'); \
	green=$$(printf '\033[0;32m'); \
	red=$$(printf '\033[0;31m'); \
	bold=$$(printf '\033[1m'); \
	white=$$(printf '\033[1;37m'); \
	reset=$$(printf '\033[0m'); \
	bar=""; \
	i=0; \
	while [ "$$i" -lt "$$filled" ]; do \
		bar="$${bar}█"; \
		i=$$((i + 1)); \
	done; \
	i=0; \
	while [ "$$i" -lt "$$empty" ]; do \
		bar="$${bar}░"; \
		i=$$((i + 1)); \
	done; \
	echo ""; \
	printf "%s╔══════════════════════════════════════════════════╗%s\n" "$$cyan" "$$reset"; \
	printf "%s║              %sTEST REPORT%s                         %s║%s\n" \
		"$$cyan" "$$bold$$white" "$$reset" "$$cyan"; \
	printf "%s╠══════════════════════════════════════════════════╣%s\n" "$$cyan" "$$reset"; \
	printf "  Total tests:  %s\n" "$$total"; \
	printf "  Passed:       %s%s%s\n" "$$green" "$$passed" "$$reset"; \
	printf "  Failed:       %s%s%s\n" "$$red" "$$failed" "$$reset"; \
	echo ""; \
	if [ "$$failed" -eq 0 ] && [ "$$status" -eq 0 ]; then \
		bar_color="$$green"; \
	else \
		bar_color="$$red"; \
	fi; \
	printf "  Progress:     %s[%s%s%s] %s%d%%%s\n" \
		"$$white" "$$bar_color" "$$bar" "$$reset" "$$bold" "$$percent" "$$reset"; \
	echo ""; \
	printf "%s╠══════════════════════════════════════════════════╣%s\n" "$$cyan" "$$reset"; \
	if [ "$$failed" -eq 0 ] && [ "$$status" -eq 0 ]; then \
		printf "║              %s✓ ALL TESTS PASSED! 🎉              %s║%s\n" "$$green$$bold" "$$reset"; \
	else \
		printf "║              %s✗ TESTS FAILED                      %s║%s\n" "$$red$$bold" "$$reset"; \
	fi; \
	printf "%s╚══════════════════════════════════════════════════╝%s\n" "$$cyan" "$$reset"; \
	echo ""; \
	exit $$status
