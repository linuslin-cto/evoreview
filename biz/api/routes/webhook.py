"""Webhook 路由。"""
import json
import os
from urllib.parse import urlparse
from flask import Blueprint, request, jsonify

from biz.platforms.gitlab.webhook_handler import slugify_url
from biz.queue.worker import (
    handle_merge_request_event,
    handle_push_event,
    handle_github_pull_request_event,
    handle_github_push_event,
    handle_gitea_pull_request_event,
    handle_gitea_push_event
)
from biz.utils.log import logger
from biz.utils.queue import handle_queue

webhook_bp = Blueprint('webhook', __name__)


@webhook_bp.route('/review/webhook', methods=['POST'])
def handle_webhook():
    if not request.is_json:
        return jsonify({'message': 'Invalid data format'}), 400
    data = request.get_json()
    if not data:
        return jsonify({"error": "Invalid JSON"}), 400

    if request.headers.get('X-Gitea-Event'):
        return handle_gitea_webhook(request.headers.get('X-Gitea-Event'), data)
    if request.headers.get('X-GitHub-Event'):
        return handle_github_webhook(request.headers.get('X-GitHub-Event'), data)
    return handle_gitlab_webhook(data)


def handle_github_webhook(event_type, data):
    github_token = os.getenv('GITHUB_ACCESS_TOKEN') or request.headers.get('X-GitHub-Token')
    if not github_token:
        return jsonify({'message': 'Missing GitHub access token'}), 400

    github_url = os.getenv('GITHUB_URL') or 'https://github.com'
    github_url_slug = slugify_url(github_url)

    logger.info(f'Received GitHub event: {event_type}')
    logger.info(f'Payload: {json.dumps(data)}')

    if event_type == "pull_request":
        handle_queue(handle_github_pull_request_event, data, github_token, github_url, github_url_slug)
        return jsonify(
            {'message': f'GitHub request received(event_type={event_type}), will process asynchronously.'}), 200
    if event_type == "push":
        handle_queue(handle_github_push_event, data, github_token, github_url, github_url_slug)
        return jsonify(
            {'message': f'GitHub request received(event_type={event_type}), will process asynchronously.'}), 200

    error_message = f'Only pull_request and push events are supported for GitHub webhook, but received: {event_type}.'
    logger.error(error_message)
    return jsonify(error_message), 400


def handle_gitlab_webhook(data):
    object_kind = data.get("object_kind")

    gitlab_url = os.getenv('GITLAB_URL') or request.headers.get('X-Gitlab-Instance')
    if not gitlab_url:
        repository = data.get('repository') or {}
        homepage = repository.get("homepage") or (data.get('project') or {}).get('web_url')
        if not homepage:
            return jsonify({'message': 'Missing GitLab URL (set GITLAB_URL or ensure webhook payload has repository.homepage)'}), 400
        try:
            parsed_url = urlparse(homepage)
            if not parsed_url.scheme or not parsed_url.netloc:
                return jsonify({'message': 'Invalid GitLab homepage URL in payload'}), 400
            gitlab_url = f"{parsed_url.scheme}://{parsed_url.netloc}/"
        except Exception as e:
            return jsonify({"error": f"Failed to parse homepage URL: {str(e)}"}), 400
    elif not gitlab_url.endswith('/'):
        gitlab_url = gitlab_url.rstrip('/') + '/'

    gitlab_token = os.getenv('GITLAB_ACCESS_TOKEN') or request.headers.get('X-Gitlab-Token')
    if not gitlab_token:
        return jsonify({'message': 'Missing GitLab access token'}), 400

    gitlab_url_slug = slugify_url(gitlab_url)
    logger.info(f'Received event: {object_kind}')
    logger.info(f'Payload: {json.dumps(data)}')

    if object_kind == "merge_request":
        handle_queue(handle_merge_request_event, data, gitlab_token, gitlab_url, gitlab_url_slug)
        return jsonify(
            {'message': f'Request received(object_kind={object_kind}), will process asynchronously.'}), 200
    if object_kind == "push":
        handle_queue(handle_push_event, data, gitlab_token, gitlab_url, gitlab_url_slug)
        return jsonify(
            {'message': f'Request received(object_kind={object_kind}), will process asynchronously.'}), 200

    error_message = f'Only merge_request and push events are supported (both Webhook and System Hook), but received: {object_kind}.'
    logger.error(error_message)
    return jsonify(error_message), 400


def handle_gitea_webhook(event_type, data):
    gitea_token = os.getenv('GITEA_ACCESS_TOKEN') or request.headers.get('X-Gitea-Token')
    if not gitea_token:
        return jsonify({'message': 'Missing Gitea access token'}), 400

    gitea_url = os.getenv('GITEA_URL') or 'https://gitea.com'
    gitea_url_slug = slugify_url(gitea_url)

    logger.info(f'Received Gitea event: {event_type}')
    logger.info(f'Payload: {json.dumps(data)}')

    if event_type == "pull_request":
        handle_queue(handle_gitea_pull_request_event, data, gitea_token, gitea_url, gitea_url_slug)
        return jsonify(
            {'message': f'Gitea request received(event_type={event_type}), will process asynchronously.'}), 200
    if event_type == "push":
        handle_queue(handle_gitea_push_event, data, gitea_token, gitea_url, gitea_url_slug)
        return jsonify(
            {'message': f'Gitea request received(event_type={event_type}), will process asynchronously.'}), 200

    error_message = f'Only pull_request and push events are supported for Gitea webhook, but received: {event_type}.'
    logger.error(error_message)
    return jsonify(error_message), 400
