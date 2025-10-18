#!/bin/bash

# Updated run script for current Stellar CLI and testnet consistency
NETWORK="testnet"

echo "1. Creating two shareholder identities"
stellar keys generate --global soro-shareholder1
stellar keys generate --global soro-shareholder2
SORO_SHAREHOLDER1_IDENTITY=$(stellar keys address soro-shareholder1)
SORO_SHAREHOLDER2_IDENTITY=$(stellar keys address soro-shareholder2)

echo "Shareholder 1: $SORO_SHAREHOLDER1_IDENTITY"
echo "Shareholder 2: $SORO_SHAREHOLDER2_IDENTITY"

echo "Funding shareholder accounts..."
curl -s "https://friendbot.stellar.org/?addr=$SORO_SHAREHOLDER1_IDENTITY" > /dev/null
curl -s "https://friendbot.stellar.org/?addr=$SORO_SHAREHOLDER2_IDENTITY" > /dev/null
echo "Shareholder accounts funded"

# Deploy a new splitter contract instance directly
echo "2. Deploying new splitter contract instance"
export SPLITTER_CONTRACT_ID=$(stellar contract deploy \
  --wasm-hash $(cat scripts/artifacts/splitter_contract_wasm_hash) \
  --source soro-wallet \
  --network $NETWORK)
printf "%s" "$SPLITTER_CONTRACT_ID" > scripts/artifacts/splitter_contract_id

echo "Splitter contract deployed at: $SPLITTER_CONTRACT_ID"

echo "3. Checking if splitter needs initialization"
# Try to get config to see if already initialized
CONFIG=$(stellar contract invoke \
  --id $SPLITTER_CONTRACT_ID \
  --source soro-wallet \
  --network $NETWORK \
  -- \
  get_config 2>/dev/null || echo "")

if [ -z "$CONFIG" ]; then
  echo "Initializing the splitter contract..."
  # Initialize with proper share format
  # The shares parameter expects a Vec<ShareDataKey> where each ShareDataKey has shareholder (Address) and share (i128)
  stellar contract invoke \
    --id $SPLITTER_CONTRACT_ID \
    --source soro-wallet \
    --network $NETWORK \
    -- \
    init \
    --admin=$(cat scripts/artifacts/soro_wallet) \
    --shares='[{"shareholder":"'${SORO_SHAREHOLDER1_IDENTITY}'","share":"8050"},{"shareholder":"'${SORO_SHAREHOLDER2_IDENTITY}'","share":"1950"}]' \
    --mutable=true || {
      echo "Failed to initialize with simple format. Trying alternative format..."
      # Alternative: Try without quotes around share values
      stellar contract invoke \
        --id $SPLITTER_CONTRACT_ID \
        --source soro-wallet \
        --network $NETWORK \
        -- \
        init \
        --admin $(cat scripts/artifacts/soro_wallet) \
        --shares '[{"shareholder":"'${SORO_SHAREHOLDER1_IDENTITY}'","share":8050},{"shareholder":"'${SORO_SHAREHOLDER2_IDENTITY}'","share":1950}]' \
        --mutable true
    }
else
  echo "Splitter already initialized with config: $CONFIG"
fi

echo "4. Checking if token needs initialization and minting 100 tokens to splitter contract"
# Try to get token info first to see if it's already initialized
TOKEN_NAME=$(stellar contract invoke \
  --id $(cat scripts/artifacts/token_contract_id) \
  --source soro-wallet \
  --network $NETWORK \
  -- \
  name 2>/dev/null || echo "")

if [ -z "$TOKEN_NAME" ]; then
  echo "Initializing token contract..."
  stellar contract invoke \
    --id $(cat scripts/artifacts/token_contract_id) \
    --source soro-wallet \
    --network $NETWORK \
    -- \
    initialize \
    --admin $(cat scripts/artifacts/soro_wallet) \
    --decimal 7 \
    --name "Custom Token" \
    --symbol "CTK"
else
  echo "Token already initialized: $TOKEN_NAME"
fi

echo "Minting tokens to splitter contract..."
stellar contract invoke \
  --id $(cat scripts/artifacts/token_contract_id) \
  --source soro-wallet \
  --network $NETWORK \
  -- \
  mint \
  --to $SPLITTER_CONTRACT_ID \
  --amount 1000000000

echo "5. Distributing tokens to shareholders"
stellar contract invoke \
  --id $SPLITTER_CONTRACT_ID \
  --source soro-wallet \
  --network $NETWORK \
  -- \
  distribute_tokens \
  --token_address $(cat scripts/artifacts/token_contract_id)

echo "6. Withdrawing allocations for each shareholder"
# First check allocations to see how much each shareholder can withdraw
echo "Checking allocations..."
ALLOCATION1=$(stellar contract invoke \
  --id $SPLITTER_CONTRACT_ID \
  --source soro-wallet \
  --network $NETWORK \
  -- \
  get_allocation \
  --shareholder $SORO_SHAREHOLDER1_IDENTITY \
  --token $(cat scripts/artifacts/token_contract_id) 2>/dev/null || echo "0")
echo "Shareholder 1 allocation: $ALLOCATION1"

ALLOCATION2=$(stellar contract invoke \
  --id $SPLITTER_CONTRACT_ID \
  --source soro-wallet \
  --network $NETWORK \
  -- \
  get_allocation \
  --shareholder $SORO_SHAREHOLDER2_IDENTITY \
  --token $(cat scripts/artifacts/token_contract_id) 2>/dev/null || echo "0")
echo "Shareholder 2 allocation: $ALLOCATION2"

# Withdraw allocations (805000000 for 80.5%, 195000000 for 19.5%)
stellar contract invoke \
  --id $SPLITTER_CONTRACT_ID \
  --source soro-shareholder1 \
  --network $NETWORK \
  -- \
  withdraw_allocation \
  --token_address $(cat scripts/artifacts/token_contract_id) \
  --shareholder $SORO_SHAREHOLDER1_IDENTITY \
  --amount 805000000

stellar contract invoke \
  --id $SPLITTER_CONTRACT_ID \
  --source soro-shareholder2 \
  --network $NETWORK \
  -- \
  withdraw_allocation \
  --token_address $(cat scripts/artifacts/token_contract_id) \
  --shareholder $SORO_SHAREHOLDER2_IDENTITY \
  --amount 195000000

echo "7. Checking final token balances"
SHAREHOLDER1_BALANCE=$(stellar contract invoke \
  --id $(cat scripts/artifacts/token_contract_id) \
  --source soro-wallet \
  --network $NETWORK \
  -- \
  balance \
  --id $SORO_SHAREHOLDER1_IDENTITY)

SHAREHOLDER2_BALANCE=$(stellar contract invoke \
  --id $(cat scripts/artifacts/token_contract_id) \
  --source soro-wallet \
  --network $NETWORK \
  -- \
  balance \
  --id $SORO_SHAREHOLDER2_IDENTITY)

echo "Shareholder 1 balance: $SHAREHOLDER1_BALANCE"
echo "Shareholder 2 balance: $SHAREHOLDER2_BALANCE"

exit 0