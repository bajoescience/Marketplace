## Marketplace

The Marketplace is a decentralized operating system that co-ordinates and economizes execution of computational work across a peer-to-peer network.

**This project is built upon the ideas presented in the Whitepaper: see whitepaper.pdf**

## Interest

**After reading the whitepaper, if you are interested in contributing or critiquing the project, you can join our community and introduce yourself:**

Discord: https://discord.gg/r9Dtwtrh

or contact me at <afiliateejoseph@gmail.com>


## Architecture

**First, we build the core protocol, which includes:**

- Messaging and Time-stamping System
- Mempool Operations
- Implementation of POC (VDF) and whiteroom committee
- Creation of blockfiles(block) and adding it to the blockchain

## Design Goals

- The core protocol exists in kernel space, so it should be as secure as possible.
- It should be fast, as it's analogous to the context switch of a kernel despite being a protocol. The more time is used during protocol execution (mempool operations, messaging, ...), the less useful work is done.
- It should be as small and simple as possible, only necessary functionalities should be in the core/kernel.

## Future

- After building the core protocol, Implementing the RISC-V based VM would be next.

## License

Licensed under:
  * Mozilla Public License, Version 2.0 (<https://www.mozilla.org/en-US/MPL/2.0/>)
  
  
### Contribution

Anyone is welcome to contribute, but make sure to read the whitepaper first and then speak to me about which what you want to do.
Find me at <afiliateejoseph@gmail.com>
