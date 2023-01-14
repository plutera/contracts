use anchor_lang::prelude::*;
use anchor_spl::token::spl_token::instruction::AuthorityType;
use anchor_spl::token::{self, Mint, SetAuthority, Token, TokenAccount, Transfer};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const DISCRIMINATOR_LENGTH: usize = 8;
const PUBLIC_KEY_LENGTH: usize = 32;
const STRING_LENGTH_PREFIX: usize = 4;
const STRING_CHAR_MULTIPLIER: usize = 4;

#[program]
pub mod contracts {

    use anchor_spl::token::transfer;

    use super::*;

    pub fn initialize_buidl(ctx: Context<InitializeBuidl>, db_id: String) -> Result<()> {
        let buidl_account: &mut Account<BuidlAccount> = &mut ctx.accounts.buidl_account;
        let owner: &Signer = &ctx.accounts.owner;
        let mint = &ctx.accounts.mint;
        let vault = &ctx.accounts.vault;

        msg!("enter ix");

        let (vault_authority, _vault_authority_bump) = Pubkey::find_program_address(
            &[
                b"authority",
                buidl_account.key().as_ref(),
                mint.key().as_ref(),
            ],
            ctx.program_id,
        );

        msg!("vault_authority: {:?}", vault_authority);

        buidl_account.owner = *owner.key;
        buidl_account.db_id = db_id;
        buidl_account.vault_account = vault.key();
        buidl_account.token = mint.key();

        token::set_authority(
            ctx.accounts.into_set_authority_context(),
            AuthorityType::AccountOwner,
            Some(vault_authority.key()),
        )?;

        Ok(())
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        // token::transfer(ctx.accounts.into_transfer_to_pda_context(), amount)?;

        let depositor = &ctx.accounts.depositor;
        let vault = &ctx.accounts.vault;
        let depoitor_token_account = &ctx.accounts.depositor_token_account;
        let token_program = &ctx.accounts.token_program;

        transfer(
            CpiContext::new(
                token_program.to_account_info(),
                Transfer {
                    from: depoitor_token_account.to_account_info(),
                    to: vault.to_account_info(),
                    authority: depositor.to_account_info(),
                },
            ),
            amount,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeBuidl<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(init, payer = owner, space = BuidlAccount::LEN)]
    pub buidl_account: Account<'info, BuidlAccount>,
    #[account()]
    pub system_program: Program<'info, System>,
    #[account(
        init_if_needed,
        payer = owner,
        seeds = [b"vault".as_ref(), buidl_account.key().as_ref(), mint.key().as_ref()],
        bump,
        token::mint = mint,
        token::authority = owner
    )]
    pub vault: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    /// CHECK: idk
    pub vault_authority: AccountInfo<'info>,
    /// CHECK: idk
    pub buidl_account: AccountInfo<'info>,
    /// CHECK: idk
    #[account(mut)]
    pub mint: AccountInfo<'info>,
    #[account(mut)]
    pub depositor: Signer<'info>,
    #[account(mut)]
    /// CHECK: idk
    pub depositor_token_account: AccountInfo<'info>,
}

#[account]
pub struct BuidlAccount {
    pub owner: Pubkey,
    pub db_id: String,
    pub vault_account: Pubkey,
    pub token: Pubkey,
}

const DB_ID_LENGTH: usize = STRING_LENGTH_PREFIX + (24 * STRING_CHAR_MULTIPLIER);

impl BuidlAccount {
    pub const LEN: usize = DISCRIMINATOR_LENGTH // discriminator
        + PUBLIC_KEY_LENGTH // owner
        + DB_ID_LENGTH // DB ID
        + PUBLIC_KEY_LENGTH // vault account
        + PUBLIC_KEY_LENGTH; // token
}

impl<'info> InitializeBuidl<'info> {
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.vault.to_account_info(),
            current_authority: self.owner.to_account_info(),
        };
        CpiContext::new(self.token_program.to_account_info(), cpi_accounts)
    }
}
