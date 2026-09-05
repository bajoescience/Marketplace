<img src="Goldcoin_img.png" alt="Goldcoin" width="150" style="vertical-align: middle;">

The Marketplace is a decentralized operating system that co-ordinates and economizes execution of computational work across a peer-to-peer network.

**This project is built upon the ideas presented in the Whitepaper: see Marketplace_whitepaper.pdf**

## Architecture

This project is divided into 7 packages which are:

**Marketplace core**

This library is the entry point of the Marketplace project. 
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

## RoadMap / Future Work

- Use an existing RISC-V Virtual Machine to build a worker
- Finish the Mempool implementation
- Implement peer-to-peer communication.
- Find and integrate a suitable decentralized storage network

## Design Goals
- Release a working binary before 1st January 2027.

### Interest

**After reading the whitepaper, if you are interested in contributing or critiquing the project, you can join our community and introduce yourself:**

Discord: https://discord.gg/WXKPyGwyGM

or contact me at <afiliateejoseph@gmail.com>

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
  
  
### Contribution

Anyone is welcome to contribute, but make sure to read the whitepaper first.
Find me at <afiliateejoseph@gmail.com>
