#!/bin/bash
/home/stakepool/solana-program-library/target/release/spl-stake-pool update w55eR1rNUjD1hLxvxsVtBDm98iPdSWTbLmTh8B1WA4k >> /home/stakepool/cron/update-stake-pool-test.log "$@" >> /home/stakepool/cron/update-stake-pool-test.log 2>> /home/stakepool/cron/update-stake-pool-test.log
wait
/home/stakepool/solana-program-library/target/release/spl-stake-pool update EEx8JxE1hhpnQw1fLwMY1aLsYdCcphZyrYC3SLSjvx5R >> /home/stakepool/cron/update-stake-pool-test.log "$@" >> /home/stakepool/cron/update-stake-pool-test.log 2>> /home/stakepool/cron/update-stake-pool-test.log
wait
solana config set --url devnet
wait
/home/stakepool/solana-program-library/target/release/spl-stake-pool update J3WCjeAhgzH4DkN2bSLSAC7K1KU5r36ooBJkSMARrNjU >> /home/stakepool/cron/update-stake-pool-dev.log "$@" >> /home/stakepool/cron/update-stake-pool-dev.log 2>> /home/stakepool/cron/update-stake-pool-dev.log
wait
solana config set --url testnet