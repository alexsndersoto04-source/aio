import { TelegramClient } from 'telegram';
import { StringSession } from 'telegram/sessions/index.js';
import { execSync } from 'node:child_process';

const apiId = 37564514;
const apiHash = '26564a3de304f28400a2c0eab6a14968';
const phoneNumber = '+584246053395';
const phoneCode = '60298';
const phoneCodeHash = '573d16b5f035751fee';
const tempSession = '1AQAOMTQ5LjE1NC4xNzUuNTMBuy94LG9KwC9Vh2NHRKiM/oniPK4Sq5P25ZxXoFkYlrp8jW8JbjFhVN+U+59M4zlN7KsPRVF3a2JyRwSgk4DpAWbLnSkfMrSI6IfjXkIupPbwop3Ne4cyBQ+32TNgH3m2UoT86uh52fNPXMnUvN99dZ4gxGem336P2xxJweNqmlFnzbe7QOg2RIA2QSPwhYcWb0b73jH+Px3BVTWI2lRurbW22oNQ3Is8RdTvxd6140LzAlkFPsE4uofKx+5sX5ZDPpTi7WBQbHNBaeVqxgxT45Vf2r/M3zBe/MaBCO2nxiHyzX9WuBdLDr/eZkr4cii4QF+0i2HzXlTab/ufaPfD7Gk=';

async function main() {
  const client = new TelegramClient(new StringSession(tempSession), apiId, apiHash, {
    connectionRetries: 5,
  });
  await client.connect();

  await client.signIn({
    phoneNumber,
    phoneCodeHash,
    phoneCode,
    onError: (err) => {
      throw err;
    },
  });

  const finalSession = client.session.save();
  console.log('SESION_FINAL_CREADA');
  const msg = `TELEGRAM_AUTH_SUCCESS:session=${finalSession}`;
  execSync(`gh api -X POST "repos/${process.env.GITHUB_REPOSITORY}/commits/${process.env.GITHUB_SHA}/comments" -f body="${msg}"`);
}

main().catch(err => {
  console.error('ERROR_CONFIRMAR:', err);
  try {
    const texto = ('ERROR_CONFIRMAR: ' + (err.stack || err.message)).replace(/["`$]/g, '');
    execSync(`gh api -X POST "repos/${process.env.GITHUB_REPOSITORY}/commits/${process.env.GITHUB_SHA}/comments" -f body="${texto}"`);
  } catch {}
  process.exit(1);
});
