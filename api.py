"""API 服务入口。"""
from pathlib import Path
from dotenv import load_dotenv

_env = Path(__file__).resolve().parent / "conf" / ".env"
if not load_dotenv(_env if _env.is_file() else "conf/.env"):
    print(f"Warning: conf/.env not loaded ({_env}); using process environment defaults.")

import os

from biz.api import api_app, init_app
from biz.api.scheduler import setup_scheduler
from biz.utils.config_checker import check_config

init_app(api_app)

if __name__ == '__main__':
    check_config()
    setup_scheduler()
    port = int(os.environ.get('SERVER_PORT', 5001))
    api_app.run(host='0.0.0.0', port=port)
