import json
from datetime import datetime
from flask import Blueprint, jsonify

from biz.api import push_review_enabled
from biz.service.review_service import ReviewService
from biz.utils.im import notifier
from biz.utils.log import logger
from biz.utils.reporter import Reporter

daily_report_bp = Blueprint('daily_report', __name__)


def _generate_daily_report() -> str | None:
    """Build daily report text; return None when there is no data."""
    start_time = datetime.now().replace(hour=0, minute=0, second=0, microsecond=0).timestamp()
    end_time = datetime.now().replace(hour=23, minute=59, second=59, microsecond=0).timestamp()

    if push_review_enabled:
        df = ReviewService().get_push_review_logs(updated_at_gte=start_time, updated_at_lte=end_time)
    else:
        df = ReviewService().get_mr_review_logs(updated_at_gte=start_time, updated_at_lte=end_time)

    logger.info(f"获取到 {len(df)} 条记录")

    if df.empty:
        logger.info("No data to process.")
        return None

    subset = [c for c in ("author", "commit_messages") if c in df.columns]
    df_unique = df.drop_duplicates(subset=subset) if subset else df
    sort_by = "author" if "author" in df_unique.columns else df_unique.columns[0]
    df_sorted = df_unique.sort_values(by=sort_by)
    commits = df_sorted.to_dict(orient="records")
    report_txt = Reporter().generate_report(json.dumps(commits))
    logger.info(f"日报生成成功，内容长度: {len(report_txt)} 字符")

    return report_txt


def daily_report_task():
    """Scheduled daily report job."""
    try:
        logger.info("开始生成日报...")
        report_txt = _generate_daily_report()
        if report_txt is None:
            return
        notifier.send_notification(content=report_txt, msg_type="markdown", title="代码提交日报")
        logger.info("日报发送成功")
    except Exception:
        logger.exception("Failed to generate daily report")


@daily_report_bp.route('/review/daily_report', methods=['GET'])
def daily_report():
    try:
        logger.info("开始生成日报...")
        report_txt = _generate_daily_report()
        if report_txt is None:
            return jsonify({'message': 'No data to process.'}), 200
        notifier.send_notification(content=report_txt, msg_type="markdown", title="代码提交日报")
        logger.info("日报发送成功")
        return jsonify({'report': report_txt})
    except Exception as e:
        logger.exception("Failed to generate daily report")
        return jsonify({'message': f"Failed to generate daily report: {e}"}), 500
