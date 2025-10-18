# Stellar Dob Distribution Contracts

Stellar Dob Distribution is a set of smart contracts & interfaces to enable split transactions and revenue sharing across multiple parties in the Stellar ecosystem. 

Stellar Dob Distribution would allow any assets (tokens) that exist on Stellar to be split in a trustless, automated, & transparent way across addresses.

## Pre-requisites

Read through [Stellar Docs](https://developers.stellar.org/docs/tools/cli/install-cli) for more info about installation and usage of stellar-cli.

## Usage

Simple deployment and initialization of the Stellar Dob Distribution contract can be done with running:

```bash
make
```

This command will execute the following steps:

1. Upload Stellar Dob Distribution contract to the network

2. Upload the token contract to the network 

3. Initialize the Stellar Dob Distribution contract with the list of addresses to split the revenue with

```json
[
    {
        "shareholder": "<Random Address>",
        "share": 8050,
    },
    {
        "shareholder": "<Random Address>",
        "share": 1950,
    }
]
```

4. Mint 100 tokens to the Stellar Dob Distribution contract using the token contract

5. Distribute the tokens to the shareholders using the Stellar Dob Distribution contract

6. Display the balances of the shareholders
