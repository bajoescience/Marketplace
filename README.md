<img src="goldcoin_img.png" alt="Goldcoin Image" width="150" style="vertical-align: middle;">

The Marketplace is a decentralized execution network where idle nodes execute useful computational tasks called **Jobs** to earn monetary rewards. This network is permissionless given that any type of idle device can be utilized as a node to earn money. **See [Marketplace Whitepaper](Marketplace_whitepaper.pdf)**

Unlike other decentralized execution networks, consumers also referred to as **employers** in the marketplace network 
do not rent computational resources. Instead, they send a compute task to be executed referred to as a **Job** directly to the network. The marketplace protocol distributes the job to randomly chosen nodes on the network in a process similar to an operating system.

### Parallel Execution

The process of sending a job to the network is asynchronous, therefore an employer thread can do other tasks or send other jobs to the network while waiting on the result of a job similar to async I/O operations. This is because the marketplace network does not block on the execution of jobs nor does it wait on the financial finalization of a job befoe the result can be used by an employer allowing for parallel execution of jobs. The marketplace acheives this using a new model called:
**PESF (Parallel Execution Synchronous Finalization) See [Marketplace Whitepaper](Marketplace_whitepaper.pdf)**

This allows employers to send multiple types of jobs in parallel without the limitation and expenses of a single rented compute resource. The employer only needs pay the exact job cost to the network. 

### Whiteroom

Internally, The Marketplace protocol acheives this by selecting a committee of nodes referred to as a **Whiteroom** chosen at random to execute a job. Each whiteroom node executes the job in a secure and sandboxed Virtual Machine. If a super majority of the whiteroom produces the same result, the job reaches observed consensus, and the result can be used immediately by the job employer. Although the financial details of the job is settled in the **Synchronous Finalization** phase.

The whiteroom committee acts like a free CPU core that executes a job on behalf of the network. This allows the rest of the network to execute other jobs in parallel. If the marketplace is likened to a World Computer, The ability of the marketplace protocol to generate a new CPU core "whiteroom" on demand for every job is the source of it's parallelism.

Each whiteroom member earns the total cost of the job, even though the employer only sends enough money to pay one node. The marketplace protocol acheives this by printing the rest of the money needed to pay the remaining nodes which is the only way the native cryptocurrency [Goldcoin(GDC)](goldcoin_img.png) is created. This ties the value of GDC to useful compuatational work.

## Interested?

**If you are interested, note that we do not yet have a working binary because this project is a work in progress**

To start, read the whitepaper, because the ideas of the marketplace is complete in the whitepaper.

After, dive into the Marketplace Core library, this is the root of the project, and a easy introduction into the marketplace workspace. 

## Design Goals

- Release a working binary before 1st January 2027.

## RoadMap / Future Work

- Use an existing RISC-V Virtual Machine to build a worker
- Finish the Mempool implementation
- Implement peer-to-peer communication.
- Find and integrate a suitable decentralized storage network

### Interest

**After reading the whitepaper, if you are interested in contributing or critiquing the project, you can join our community and introduce yourself:**

Discord: https://discord.gg/WXKPyGwyGM

or contact me at <afiliateejoseph@gmail.com>

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
  
  
### Contribution

Anyone is welcome to contribute, but make sure to read the whitepaper first.
Then read [CONTRIBUTING](CONTRIBUTING.md)

God bless.
