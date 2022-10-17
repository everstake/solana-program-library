#!/bin/bash
/home/stakepool/solana-program-library/target/release/spl-stake-pool update w55eR1rNUjD1hLxvxsVtBDm98iPdSWTbLmTh8B1WA4k >> /home/stakepool/cron/update-stake-pool-test.log "$@" >> /home/stakepool/cron/update-stake-pool-test.log 2>> /home/stakepool/cron/update-stake-pool-test.log
/home/stakepool/solana-program-library/target/release/spl-stake-pool update EEx8JxE1hhpnQw1fLwMY1aLsYdCcphZyrYC3SLSjvx5R >> /home/stakepool/cron/update-stake-pool-test.log "$@" >> /home/stakepool/cron/update-stake-pool-test.log 2>> /home/stakepool/cron/update-stake-pool-test.log
solana config set --url devnet
/home/stakepool/solana-program-library/target/release/spl-stake-pool update EEx8JxE1hhpnQw1fLwMY1aLsYdCcphZyrYC3SLSjvx5R >> /home/stakepool/cron/update-stake-pool-dev.log "$@" >> /home/stakepool/cron/update-stake-pool-dev.log 2>> /home/stakepool/cron/update-stake-pool-dev.log
solana config set --url testnet