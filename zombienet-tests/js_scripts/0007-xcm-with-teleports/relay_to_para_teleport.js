const { decodeAddress } = require("@polkadot/keyring");
const { BN, u8aToHex } = require('@polkadot/util');

const { submitExtrinsic, receivedEvents } = require('zkv-lib');

const ReturnCode = {
    Ok: 1,
    WrongBalance: 2,
    ExtrinsicUnsuccessful: 3,
    FailedSavingFile: 4,
};

async function run(nodeName, networkInfo, args) {
    const { wsUri, userDefinedTypes } = networkInfo.nodesByName[nodeName];
    const api = await zombie.connect(wsUri, userDefinedTypes);

    const keyring = new zombie.Keyring({ type: 'sr25519' });
    const bob = keyring.addFromUri('//Bob');

    // Collect Bob's free balance
    let balance_bob_pre = (await api.query.system.account(bob.address))["data"]["free"];
    console.log('Bob\'s balance: ' + balance_bob_pre.toHuman());

    const amount = args[0];
    const benef = args[1]; // u8aToHex(decodeAddress(args[1]));

    // 1. Create an XCM teleport extrinsic, teleporting _amount_ tokens to _benef_
    const dest = {
        V4: {
            parents: '0',
            interior: {
                X1: [{ Parachain: 1999 }],
            },
        },
    };
    const beneficiary = {
        V4: {
            parents: '0',
            interior: {
                X1: [{
                    AccountId32: {
                        network: null,
                        id: benef,
                    },
                }]
            },
        },
    };
    const assets = {
        V4: [{
            id: {
                parents: 0,
                interior: {
                    Here: '',
                },
            },
            fun: {
                Fungible: amount,
            },
        }],
    };

    const fee_asset_item = '0';
    const weight_limit = 'Unlimited';
    const teleport = await api.tx.xcmPallet.teleportAssets(dest, beneficiary, assets, fee_asset_item);

    if (!receivedEvents(await submitExtrinsic(api, teleport, bob, BlockUntil.InBlock, undefined))) {
        return ReturnCode.ExtrinsicUnsuccessful;
    }

    // 2. Verify the cost of the teleport above

    // Get the updated balances
    balance_bob_post = (await api.query.system.account(bob.address))["data"]["free"];
    console.log('Bob\'s balance after tx: ' + balance_bob_post.toHuman());

    let paid = balance_bob_pre.sub(balance_bob_post);

    if (paid.lte(new BN(amount, 10))) {
        console.log("Paid less than the teleport amount: " + paid.toString());
        return ReturnCode.WrongBalance;
    }

    console.log("Teleport test succeeded! Paid: " + paid.toString());

    // encoded call data: 0x0007745468616e6b20796f7520666f7220796f757220706174726f6e61676521

    return ReturnCode.Ok;
}

module.exports = { run }
