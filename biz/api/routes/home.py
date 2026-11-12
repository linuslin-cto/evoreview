"""
首页路由模块
"""
from pathlib import Path

from flask import Blueprint, send_from_directory

home_bp = Blueprint('home', __name__)

_ASSETS_DIR = Path(__file__).resolve().parents[3] / "assets"


@home_bp.route('/')
def home():
    return """<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <title>獬豸</title>
  <link rel="icon" href="/assets/xiezhi-logo.png" type="image/png" />
  <style>
    body { font-family: sans-serif; text-align: center; margin-top: 4rem; color: #1a2332; }
    img { width: 120px; height: 120px; border-radius: 20px; object-fit: cover; }
  </style>
</head>
<body>
  <img src="/assets/xiezhi-logo.png" alt="獬豸" />
  <h2>獬豸 API 服务运行中。</h2>
</body>
</html>
"""


@home_bp.route('/assets/<path:filename>')
def assets(filename):
    return send_from_directory(_ASSETS_DIR, filename)
