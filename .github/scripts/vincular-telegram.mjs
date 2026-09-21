import { TelegramClient } from 'telegram';
import { StringSession } from 'telegram/sessions/index.js';
import { execSync } from 'node:child_process';

const apiId = 37564514;
const apiHash = '26564a3de304f28400a2c0eab6a14968';
const phoneNumber = '+584246053395';

async function main() {
  const client = new TelegramClient(new StringSession(''), apiId, apiHash, {
    connectionRetries: 5,
  });
  await client.connect();
  const res = await client.sendCode({ apiId, apiHash }, phoneNumber);
  console.log('CODIGO_ENVIADO');
  const msg = `TELEGRAM_CODE_SENT:phoneCodeHash=${res.phoneCodeHash}&session=${client.session.save()}`;
  execSync(`gh api -X POST "repos/${process.env.GITHUB_REPOSITORY}/commits/${process.env.GITHUB_SHA}/comments" -f body="${msg}"`);
}

main().catch(err => {
  console.error('ERROR:', err);
  execSync(`gh api -X POST "repos/${process.env.GITHUB_REPOSITORY}/commits/${process.env.GITHUB_SHA}/comments" -f body="ERROR_TG: ${err.message}"`);
});
