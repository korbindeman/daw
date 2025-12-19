.PHONY: dev install clean help

# Default target
dev: install
	cd crates/tauri && bun tauri dev

# Install node modules
install:
	cd crates/tauri && bun install

# Clean build artifacts
clean:
	cd crates/tauri && bun run clean || true
	cargo clean

# Show help
help:
	@echo "Available targets:"
	@echo "  dev     - Install dependencies and run the Tauri development server"
	@echo "  install - Install node modules using bun"
	@echo "  clean   - Clean build artifacts"
	@echo "  help    - Show this help message"
