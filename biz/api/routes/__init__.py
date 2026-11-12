"""
路由注册模块
"""
from biz.api.routes import daily_report, dashboard, home, webhook


def register_routes(app):
    """
    注册所有路由到 Flask 应用
    """
    app.register_blueprint(home.home_bp)
    app.register_blueprint(daily_report.daily_report_bp)
    app.register_blueprint(webhook.webhook_bp)
    app.register_blueprint(dashboard.dashboard_bp)

    @app.after_request
    def add_cors_headers(response):
        origin = app.config.get("CORS_ORIGIN") or "*"
        response.headers["Access-Control-Allow-Origin"] = origin
        response.headers["Access-Control-Allow-Headers"] = "Content-Type, Authorization, X-Auth-Token"
        response.headers["Access-Control-Allow-Methods"] = "GET, POST, OPTIONS"
        return response

    @app.route("/api/<path:_any>", methods=["OPTIONS"])
    def api_options(_any):
        return ("", 204)
