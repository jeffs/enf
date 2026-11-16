# Lint and test.
check: lint test

test:
    cargo test

# Clippy flags apply in order, so groups precede the lints they override.
#
# This is stricter than the set in Cargo.toml, which is comfortable during
# active development. This set is meant to pass before merging to trunk.
# These are deny rather than warn so `just lint` returns bad status on violations.
lint:
    cargo clippy --all-targets -- \
        --deny warnings \
        --deny unsafe_code \
        --deny clippy::cargo \
        --deny clippy::nursery \
        --deny clippy::pedantic \
        --deny clippy::multiple_crate_versions \
        --deny clippy::allow_attributes \
        --deny clippy::allow_attributes_without_reason \
        --deny clippy::fn_params_excessive_bools \
        --deny clippy::indexing_slicing \
        --deny clippy::let_underscore_must_use \
        --deny clippy::redundant_clone \
        --deny clippy::unneeded_field_pattern \
        --deny clippy::wildcard_enum_match_arm \
