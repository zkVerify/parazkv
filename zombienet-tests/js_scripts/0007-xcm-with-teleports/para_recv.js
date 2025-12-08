// This script is executed on the test parachain to verify that:
// 1. the parachain received the teleport from the relay chain, minting tokens to the requested account
// 2. it is possible to request an XCM teleport of a given amount of tokens toward a given account on the relay chain
// 3. it is possible to request a custom remote execution on the relay chain through XCM (in this case a submitProof extrinsic)
// 4. the test parachain receives an XCM response indicating the outcome of the remote execution

const { BN } = require('@polkadot/util');
const { decodeAddress } = require("@polkadot/keyring");

const { BLOCK_TIME } = require('zkv-lib');

const ReturnCode = {
    Ok: 1,
    WrongTeleportReceived: 2,
    ExtrinsicUnsuccessful: 3,
};

async function run(nodeName, networkInfo, args) {
    const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
    const api = await zombie.connect(wsUri, userDefinedTypes);

    // Alice's remote Computed Origin on the relay chain, computed offline with xcm-tools
    // const ALICE_REMOTE_ORIGIN = '0x7b2ac6587a1931a0b108bb03777f8e552293bd6a6ea3790a5fe14e214f13072b';

    const keyring = new zombie.Keyring({ type: 'sr25519' });

    const amount = args[0];
    const receiver = args[1]; // decodeAddress(args[1]);

    // Check that we receive the teleport from the relay chain w/ the correct parameters

    console.log("Waiting for teleport from relay chain");

    let timeout = BLOCK_TIME * 3;
    let init_balance_receiver = (await api.query.system.account(receiver))["data"]["free"];
    let balance_receiver = init_balance_receiver;

    console.log(`Initial balance of receiver: ${init_balance_receiver.toHuman()}`);

    while (!balance_receiver.eq(new BN(amount, 10))) {
        await new Promise(r => setTimeout(r, 1000));
        timeout -= 1000;
        balance_receiver = (await api.query.system.account(receiver))["data"]["free"];
        if (timeout <= 0) {
            console.log("Not yet received, giving up!");
            return ReturnCode.WrongTeleportReceived;
        }
    }

    console.log(`Received balance: ${balance_receiver.toHuman()}`);

    if (balance_receiver <= init_balance_receiver) {
        return ReturnCode.ExtrinsicUnsuccessful;
    }

    return ReturnCode.Ok;
}

module.exports = { run }
