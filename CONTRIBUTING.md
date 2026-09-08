## PREREQUISITES

- Read the [Marketplace Whitepaper](Marketplace_whitepaper.pdf)

First of all, thank you for taking interest in the Marketplace Project. It is only with your help that the Marketplace can be helpful to so many people.

## Marketplace Architecture

**This is a brief introduction into the code structure of the Marketplace project**
This project is divided into 7 packages which are:

**Marketplace core**

This is the entry point and root library of the Marketplace project. 
It functionality includes:

- Initializing a new node and it's state.
- Handling Messages to and from the Network.
- Handling asynchronous operations.

**Marketplace ledger**

This library is responsible for handling state changes.
This includes:

- Blockchain Ledger (Dead state).
- Mempool (Live State).
- Transaction input/ouput State.

**Marketplace primitives**

This library is responsible for the primitive structures as described in the 
marketplace whitepaper.
This includes the following:

- Work Pointer
- Whiteroom
- Result Pointer
- Contract

**Marketplace wallet**

This library is responsible for the cryptographic primitives used in the 
marketplace architecture, which includes the following:

- Public Key Cryptography
- Digital Signature
- Verifiable Delay Function (VDF)
- Verifiable Random Function (VRF)
- Whiteroom Proofs
- Account Token

**Marketplace worker**

This library is responsible for compute task execution. 

Nodes execute compute bount tasks on a RISC-V Virtual Machine

**Marketplace p2p**

This library is responsible for messaging, and peer management.

**Marketplace helper**

This library defines useful functions and structures that are used 
throughout the marketplace project.

## How Can You Help?

### Issues

If you have encountered any bug, or have a new feature you might want to implement in mind, [please fill an issue] 
or [start a discussion] before any PR. We would like to discuss and reach consensus on the implementation, style and correctness of the code so that we don't waste each others time.

We really want to be focused and consider any new features carefully before committing to it, and making sure it aligns with marketplace goals.

### Pull Request

**Before creating a PR, it is neccesary to follow the marketplace PR guidelines described as follows:**

- Always [start a discussion] to discuss new features

- Every PR must follow Test Driven Development, where every new feature must have working tests.
If any previous tests fail, the code has to be corrected, or the test itself rewritten (new features).

- Every PR must reference an issue, the PR description should make it clear exactly what your code and how it addresses the referenced issue to increase the chances of your PR being accepted. The accepted format of a PR heading is as follows:
  **[Marketplace Library] Short PR description (Issue number)** 
  for issues spanning a single library 
  or  
  **[Marketplace Library / Marketplace Library / ...] Short PR description (Issue number)**
  for issues spanning multiple libraries

- There must be an ideal amount of documentation and comments so that the code can be human readable, and understandable by anyone.

- Respect all community members, and do not insult other contributors and maintainers

## Community

Introduce yourself to the community by joining the [Marketplace discord](https://discord.gg/WXKPyGwyGM) and sending
a message. We'll be very glad to welcome you.

God bless.
