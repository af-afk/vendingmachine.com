
# Vending Machine

This repo is an example of how to use a Chainlink VRF call with a price feed with Arbitrum
Stylus.

There are a couple of problems with this code, namely that it's difficult to distribute
the NFTs this way, since it's difficult to bound the gas consumption, or to even estimate
it, but for this toy implementation this might be how you would do something similar.

My advice to programmers implementing this pattern is to follow up with a crank, and to
simulate the contract by setting a storage field flag, and to bound the range of sends
that way.

Despite this, we protect this contract by allowing a failure to take place inside the code
when sending a NFT. That failure might be due to the other contract not implementing the
NFT callback function.

## Building

	make build

## Testing

	./tests.sh

## Architecture

```mermaid
C4Container

Person(user, "User", "Requests using the vending machine.")

Container_Boundary(frontend, "Vending machine") {
    System(queue, "Queue for requests for a token", "Added to when a user requests to receive a NFT.")
    System(fundingRefund, "Funding refund refunds users their fees for purchasing.", "A fee is needed for Chainlink direct funding with the native token. We need to refund users a portion of the global amount invested by everyone without punishing a specific user for their fees.")
    System(frontend, "Frontend to request a NFT", "Adds to the queue for retrieval. Schedules a request.")
    System(payouts, "Sends NFTs from the pool.", "Sends NFTs that were bought by going through the queue.")
}

Container_Boundary(chainlink, "Chainlink services") {
    System(chainlinkVrf, "Chainlink VRF", "Generates randomness, then triggers a callback.")
    System(chainlinkPriceFeed, "Chainlink price feed", "Gets the price of an asset from its feeds.")
}

Rel(frontend, chainlinkVrf, "Requests randomness")
Rel(chainlinkVrf, payouts, "Requests a payout.")
Rel(payouts, chainlinkPriceFeed, "Gets the price of the asset at the time.")

Rel(user, frontend, "Locks up their token to the NFT contract for a purchase.")
```

## User stories

### Requesting a purchase from the contract

```mermaid
flowchart TD
    User((Buyer))
    -->|Locks up their ETH, paying a fee for Chainlink to respond to with VRF callback.| Frontend[Frontend]
    -->|Stores the request on a queue alongside the ETH they supplied.| Queue[Queue]
    -->|Makes request to the Chainlink VRF request with direct funding.| ChainlinkVRF[Chainlink VRF]
    -->|Responds with some randomness.| PayoutsVRFResp["Payouts (VRF response)"]
    -->|Requests the price of ETH at that time.| ChainlinkPrices[Chainlink Price Feeds]
    -->|Takes each request off the queue, uses the randomness to know which NFT to send based on the tier.| Payouts
    Frontend
    -->|Sends info on fees contributed.| FundingRefund[Funding refund]
    PayoutsVRFResp
    -->|Notifies Funding Refund that refunds are needed.| FundingRefund
    -->|Redistributes the flat fee paid by each user amongst themselves, as presumably only a little was eneded for direct funding.| User
    Payouts -->|Sends the user NFTs earned.| User
```


### Refunding users UX

We need to take fees from users who participate in this system, so that we can use
Chainlink's direct funding model with their VRF. To do this, we take a flat fee from
everyone, which is the fee that's needed by Chainlink. After the first user, we start to
record amounts deposited in a shared pool, which is used to refund everyone equally from
the amount paid. This way, we don't punish the first user for being first.

```mermaid
flowchart LR
    PayoutsVRFResp[Payouts (VRF response)]
    -->|"Notifies it's time for fee refunding."| FundingRefund[Funding refund]
    -->|Refunds user their flat portion of the VRF fee.| User((Buyer))
```
