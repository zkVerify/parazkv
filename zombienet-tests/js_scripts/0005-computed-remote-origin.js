const { blake2AsU8a, encodeAddress } = require('@polkadot/util-crypto');
const { decodeAddress } = require("@polkadot/keyring");
const { u8aConcat, u8aToHex, assert } = require('@polkadot/util');

const ReturnCode = {
    Ok: 1,
    ErrNonMatchingComputedRemoteOrigin: 2,
};

async function run(nodeName, networkInfo, args) {
    // --- Inputs ---
    const ALICE_SS58_ADDRESS = '5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY';
    const PARACHAIN_SS58_PREFIX = 0; // Polkadot prefix
    // MultiLocation V5 Discriminator (0x04) + Parents=1 (0x01) + X1 (0x01)
    const HEADER_BYTES = new Uint8Array([0x04, 0x01, 0x01]);
    // AccountId32 (0x00) + Network=None (0x00)
    const JUNCTION_TYPE_BYTES = new Uint8Array([0x00, 0x00]);
    // -----------------

    // 1. Get Alice's Public Key (32 bytes)
    const alicePublicKey = decodeAddress(ALICE_SS58_ADDRESS);

    // 2. Concatenate all parts to form the full SCALE-encoded MultiLocation
    const encodedMultiLocation = u8aConcat(
        HEADER_BYTES,
        JUNCTION_TYPE_BYTES,
        alicePublicKey
    );

    // 3. Compute the Blake2-256 hash (32 bytes)
    const computedOriginHash = blake2AsU8a(encodedMultiLocation, 256);

    // 4. Convert the hash to the Parachain's SS58 address
    const computedOriginSS58 = encodeAddress(computedOriginHash, PARACHAIN_SS58_PREFIX);

    console.log(`Computed Origin (SS58): ${computedOriginSS58}`);

    let computedOrigin = u8aToHex(decodeAddress(computedOriginSS58));
    console.log(`Decoded Computed Remote Origin (SS58): ${computedOrigin}`);

    if (computedOrigin != "0x7b2ac6587a1931a0b108bb03777f8e552293bd6a6ea3790a5fe14e214f13072b") {
        return ReturnCode.ErrNonMatchingComputedRemoteOrigin;
    }

    return ReturnCode.Ok;
}

module.exports = { run }