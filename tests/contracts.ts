import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import {
  createMint,
  TOKEN_PROGRAM_ID,
  mintTo,
  getAccount,
  getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";
import { assert } from "chai";
import { Contracts } from "../target/types/contracts";

const DB_ID = "eriqih";

describe("contracts", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.Contracts as Program<Contracts>;
  const connection = anchor.getProvider().connection;
  const userWallet = anchor.workspace.Contracts.provider.wallet;

  it("Initialize Build", async () => {
    const mint = await createMint(
      connection,
      userWallet.payer,
      userWallet.publicKey,
      userWallet.publicKey,
      6
    );

    const userAta = await getOrCreateAssociatedTokenAccount(
      connection,
      userWallet.payer,
      mint,
      userWallet.publicKey
    );

    await mintTo(
      connection,
      userWallet.payer,
      mint,
      userAta.address,
      userWallet.publicKey,
      1000
    );

    const buidlAccount = anchor.web3.Keypair.generate();

    const [vaultPDAAddress] = anchor.web3.PublicKey.findProgramAddressSync(
      [
        Buffer.from("vault"),
        buidlAccount.publicKey.toBuffer(),
        mint.toBuffer(),
      ],
      program.programId
    );

    const [vaultAuthorityAddress] =
      anchor.web3.PublicKey.findProgramAddressSync(
        [
          Buffer.from("authority"),
          buidlAccount.publicKey.toBuffer(),
          mint.toBuffer(),
        ],
        program.programId
      );

    const tx = await program.methods
      .initializeBuidl(DB_ID)
      .accounts({
        owner: userWallet.publicKey,
        mint: mint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
        buidlAccount: buidlAccount.publicKey,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        vault: vaultPDAAddress,
      })
      .signers([buidlAccount])
      .rpc();

    console.log(tx);

    await mintTo(
      connection,
      userWallet.payer,
      mint,
      vaultPDAAddress,
      userWallet.publicKey,
      1000
    );

    let fetchedVault = await getAccount(connection, vaultPDAAddress);

    const buidlAccountData = await program.account.buidlAccount.fetch(
      buidlAccount.publicKey
    );

    assert.equal(
      buidlAccountData.vaultAccount.toString(),
      vaultPDAAddress.toString()
    );
    assert.equal(
      buidlAccountData.owner.toString(),
      userWallet.publicKey.toString()
    );
    assert.equal(buidlAccountData.token.toString(), mint.toString());
    assert.equal(buidlAccountData.dbId, DB_ID);

    assert.equal(fetchedVault.amount.toString(), "1000");
    assert.equal(
      fetchedVault.owner.toString(),
      vaultAuthorityAddress.toString()
    );
  });
});
