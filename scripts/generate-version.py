#!/usr/bin/env python3
import sys

def main():
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <current_version> <major|minor|patch>", file=sys.stderr)
        sys.exit(1)
    
    current_version = sys.argv[1]
    bump_type = sys.argv[2]
    
    if bump_type not in ("major", "minor", "patch"):
        print(f"Error: bump type must be 'major', 'minor', or 'patch'. Got: {bump_type}", file=sys.stderr)
        sys.exit(1)
    
    # Parse version: v1.2.3 → major=1, minor=2, patch=3
    version = current_version.lstrip("v")
    parts = version.split(".")
    
    try:
        major = int(parts[0])
        minor = int(parts[1])
        patch = int(parts[2])
    except (IndexError, ValueError):
        print(f"Error: Invalid version format '{current_version}'. Expected v#.#.#", file=sys.stderr)
        sys.exit(1)
    
    # Bump based on input
    if bump_type == "major":
        major += 1
        minor = 0
        patch = 0
    elif bump_type == "minor":
        minor += 1
        patch = 0
    elif bump_type == "patch":
        patch += 1
    
    new_tag = f"v{major}.{minor}.{patch}"
    print(new_tag)

if __name__ == "__main__":
    main()
