// Message has different variants for all types
// This includes the following
// Job: To create a new job on network, 
// Result: To add job results to an existing job in network,
// Block: To add a new Block to a job in the network,
// Confirm: To confirm the chosen block.
#[derive(Debug)]
pub struct Message<T>
{
    pub payload: T,
}