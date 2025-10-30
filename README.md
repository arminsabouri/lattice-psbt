# Lattice-PSBT

A data structure that models PSBTs as a [semilattice](https://en.wikipedia.org/wiki/Semilattice), enabling non-conflicting fragments to merge deterministically for eventual consistency in collaborative transaction construction.

Note: This is a work in progress.

## The Problem

[BIP-174](https://github.com/bitcoin/bips/blob/master/bip-0174.mediawiki) defines a partially signed transaction format, enabling one party to construct a transaction and multiple parties to cooperatively sign it.
[BIP-370](https://github.com/bitcoin/bips/blob/master/bip-0370.mediawiki) improves on this by defining how inputs and outputs can be added to a PSBT after its creation -- useful for coordinator-less collaborative protocols like Payjoin (BIP-77/78).

However in other collaborative protocols, participants discover transaction components independently and without a central coordinator to create the transaction. Each peer needs to converge on the same transaction to sign, yet existing PSBT rules don’t define how unordered independently learned fragments should be combined. As a result, developers have resorted to ad-hoc extensions and off-spec solutions.

## The Solution

We propose a relaxation of BIP-370 that defines merge semantics for partially ordered PSBT fragments.
This approach specifies deterministic rules for joining independently learned transaction components so peers reach a consistent, ordered PSBT without coordination.

By modeling a PSBT as a state-based conflict-free replicated data type (CRDT) wallets can merge updates as they learn new inputs, outputs, or other metadata. This ensures that collaboration produces a coherent transaction state regardless of message ordering or duplication.
