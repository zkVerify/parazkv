const { BN, compactAddLength, u8aToHex } = require('@polkadot/util');
const { decodeAddress } = require("@polkadot/keyring");

const fs = require('fs');

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
    const alice = keyring.addFromUri('//Alice');

    // Collect Alice's free balance
    let balance_alice_pre = (await api.query.system.account(alice.address))["data"]["free"];
    console.log('Alice\'s balance: ' + balance_alice_pre.toHuman());

    const amount = args[0];
    const benef = args[1]; // DIMITRI = "5G6DXujt47QrKcrZ1wAbr3SJxXRZLKkkv7vzfSHHbDtJzZZo";

    console.log(`benef = 0x${Array.from(decodeAddress(benef), b => b.toString(16).padStart(2, "0")).join("")}`);

    // 1. Create an XCM teleport extrinsic, teleporting _amount_ tokens to _benef_
    const dest = {
        V5: {
            parents: '0',
            interior: {
                X1: [{ Parachain: 1599 }],
            },
        },
    };
    const beneficiary = {
        V5: {
            parents: '0',
            interior: {
                X1: [{
                    AccountId32: {
                        network: null,
                        id: decodeAddress(benef),
                    },
                }]
            },
        },
    };
    const assets = {
        V5: [{
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

    if (!receivedEvents(await submitExtrinsic(api, teleport, alice, BlockUntil.InBlock, undefined))) {
        return ReturnCode.ExtrinsicUnsuccessful;
    }

    // 2. Verify the cost of the teleport above

    // Get the updated balances
    balance_alice_post = (await api.query.system.account(alice.address))["data"]["free"];
    console.log('Alice\'s balance after tx: ' + balance_alice_post.toHuman());

    let paid = balance_alice_pre.sub(balance_alice_post);

    if (paid.lte(new BN(amount, 10))) {
        console.log("Paid less than the teleport amount: " + paid.toString());
        return ReturnCode.WrongBalance;
    }

    return ReturnCode.Ok;
}

module.exports = { run }
