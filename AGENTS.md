# AI Development Notes

- Run `cargo fmt --all` before committing.
- Run `cargo test` to execute the workspace test suite.
- When decoding SPL Token mint accounts, ensure the account data length is at least `spl_token::state::Mint::LEN` to avoid panics from `unpack_from_slice`.
