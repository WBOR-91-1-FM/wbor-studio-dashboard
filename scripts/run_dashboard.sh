#!/bin/bash

# Full path to project dir
PROJECT_DIR="/Users/wborguest/wbor-studio-dashboard"

# Navigate to the project directory
cd "$PROJECT_DIR" || exit

# Run the project with cargo
RUST_LOG=wbor_studio_dashboard cargo run --release >>"$PROJECT_DIR/project.log" 2>&1
