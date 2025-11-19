BlockUntil = {
    InBlock: 'InBlock',

    Finalized: 'Finalized',
};

exports.BlockUntil = BlockUntil;

let api = null;

const BLOCK_TIME = 6000;  // block time in milliseconds
exports.BLOCK_TIME = BLOCK_TIME;

async function _handleTransactionLifecycle(api, sendFunction, blockUntil, filter) {
    let transactionSuccessEvent = false;
    let done = false;
    let max_retries = 5;
    let hasBeenReady = false;
    if (filter === undefined) {
        console.log("No filtering");
        filter = (_event) => true;
    }

    let retVal = -1;
    while (!done && max_retries > 0) {
        retVal = await new Promise(async (resolve, reject) => {
            const unsub = await sendFunction(({ events: records = [], status }) => {
                let blockHash = null;
                if (status.isReady) {
                    hasBeenReady = true;
                }
                else if (status.isInBlock) {
                    blockHash = status.asInBlock;
                    console.log(`Transaction included at blockhash ${blockHash}`);
                    records.forEach(({ event: { method, section } }) => {
                        if (section == "system" && method == "ExtrinsicSuccess") {
                            transactionSuccessEvent = true;
                        }
                    });
                    if (blockUntil === BlockUntil.InBlock) {
                        done = true;
                    }
                }
                else if (status.isFinalized) {
                    console.log(`Transaction finalized at blockhash ${status.asFinalized}`);
                    if (blockUntil === BlockUntil.Finalized) {
                        done = true;
                    }
                }
                else if (status.isInvalid) {
                    console.log("Transaction marked as invalid");
                    done = true;
                    if (hasBeenReady) {
                        reject("retry");
                    }
                }
                else if (status.isError) {
                    done = true;
                    console.log("ERROR: Transaction status.isError");
                }
                if (done) {
                    unsub();
                    if (transactionSuccessEvent) {
                        resolve([blockHash, records]);
                    } else {
                        reject("ExtrinsicSuccess has not been seen");
                    }
                }
            }).catch(
                error => {
                    console.log(`Sending extrinsic failed with error: ${error}`);
                    if (error.code === 1014) { // priority too low error
                        reject("retry");
                    } else {
                        reject(error);
                    }
                }
            );
        }).then(
            ([blockHash, records]) => {
                console.log(`Transaction successfully processed [${blockHash}]: ${records}`);
                return {
                    block: blockHash,
                    events: records.map((record) => record.event).filter(filter)
                };
            },
            async function (error) {
                if (error !== "retry") {
                    console.log("Not retrying!");
                    done = true;
                    return -1;
                }
                console.log("Transaction should be resubmitted, waiting for empty mempool...");
                max_retries -= 1;
                done = false;
                await waitForEmptyMempool(api);
            }
        );
    }

    return retVal;
}

async function submitExtrinsic(api, extrinsic, signer, blockUntil, filter) {
    // Create a function that encapsulates the `signAndSend` call.
    const sendFunction = (callback) => extrinsic.signAndSend(signer, callback);

    // Delegate all the hard work to the handler.
    return _handleTransactionLifecycle(api, sendFunction, blockUntil, filter);
}

async function waitForEmptyMempool(api) {
    let pending = 0;
    do {
        await new Promise(r => setTimeout(r, BLOCK_TIME));
        pending = await api.rpc.author.pendingExtrinsics();
        console.log(`${pending.length} extrinsics pending in the mempool`);
    } while (pending.length > 0);
}

exports.submitExtrinsic = async (api, extrinsic, signer, blockUntil, filter) => {
    return await submitExtrinsic(api, extrinsic, signer, blockUntil, filter);
}

exports.receivedEvents = (data) => {
    let events = Array.isArray(data) ? data : data.events;
    return Array.isArray(events) && events.length > 0;
}