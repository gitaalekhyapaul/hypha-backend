# Approved Authorization Guest

`approved-v1.elf` is the public SP1 authorization interpreter, not a proof or
private witness. Runtime preparation installs this exact approved artifact.

- Size: `386088` bytes.
- SHA-256: `817b8689719d2079756160e56fe779b96f8e8194b77f68b5cca1c82fcb664a41`.
- Verification-key ID: `sp1-vk-v1:c80994ad25358482755752c3068dc51c61f52d136e4eaf4cc9da8277239008a1`.
- SP1 program-key hash: `0x00402e4ea52d0a241bee8bd42cdffc95df4a42f460dbf4ac48c90b99a78aa077`.

## Provenance

Recovered on 2026-10-15 from the public guest retained in the NUS VM's
`target/elf-compilation/riscv64im-succinct-zkvm-elf/release/` directory.
A network-disabled public-only SP1 key derivation matched both identifiers in
`services/verifierd/tests/fixtures/retained-plonk-v1/`, whose proof source commit
is `35db2707ff1b5627c1bc0779120eea8fabc5d455`.
The recovery does not establish byte-for-byte source build reproducibility.

This preserves the already trusted program identity. It does not approve a new
guest or change PolicyIR semantics. Policy contents and grant values remain
proof inputs and do not change this interpreter artifact.

## Updating The Guest

Do not replace this file with an arbitrary local rebuild. A guest update needs
an explicit program-key approval, a new pinned artifact and checksum, matching
verifier trust, and real-proof validation. Runtime preparation checks the
checksum and derives both key identifiers before installing any runtime files.
The child repeats the identity check before receiving private input.
