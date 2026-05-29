npm install -g openclaw@latest
# or: pnpm add -g openclaw@latest

openclaw onboard --install-daemon
mkdir -p ~/code/openclaw-local
# Copy templates/agent-first/flake.nix from the nix-openclaw repo
git clone https://github.com/openclaw/openclaw.git
cd openclaw-builder

pnpm install
pnpm ui:build # auto-installs UI deps on first run
pnpm build

pnpm openclaw onboard --install-daemon

# Dev loop (auto-reload on TS changes)
pnpm gateway:watch
openclaw onboard --install-daemon

openclaw gateway --port 8080 --verbose

# Send a message
openclaw message send --to +2348132736003 --message "Hello from OpenClaw"

# Talk to the assistant (optionally deliver back to any connected channel: WhatsApp/Telegram/Slack/Discord/Google Chat/Signal/iMessage/BlueBubbles/Microsoft Teams/Matrix/Zalo/Zalo Personal/WebChat)
openclaw agent --message "Ship checklist" --thinking high
