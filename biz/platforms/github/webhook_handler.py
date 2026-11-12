import os
import re
import time

import fnmatch

from biz.platforms.http_utils import platform_get, platform_post
from biz.utils.log import logger



def filter_changes(changes: list):
    """Keep supported extensions and diff/new_path fields only."""
    supported_extensions = [
        ext.strip()
        for ext in os.getenv('SUPPORTED_EXTENSIONS', '.java,.py,.php').split(',')
        if ext.strip()
    ]
    
    not_deleted_changes = []
    for change in changes:
        if change.get('status') == 'removed':
            logger.info(f"Detected file deletion via status field: {change.get('new_path')}")
            continue
            
        diff = change.get('diff', '')
        if diff:
            diff_header_match = re.match(r'@@ -\d+,\d+ \+0,0 @@', diff)
            if diff_header_match:
                diff_lines = diff.split('\n')[1:]
                # empty diff body must not count as deletion (all([]) == True)
                if diff_lines and all(line.startswith('-') or not line for line in diff_lines):
                    logger.info(f"Detected file deletion via diff pattern: {change.get('new_path')}")
                    continue
                    
        not_deleted_changes.append(change)
    
    logger.info(f"SUPPORTED_EXTENSIONS: {supported_extensions}")
    logger.info(f"After filtering deleted files: {not_deleted_changes}")
    
    filtered_changes = [
        {
            'diff': item.get('diff', ''),
            'new_path': item['new_path'],
            'additions': item.get('additions', 0),
            'deletions': item.get('deletions', 0),
        }
        for item in not_deleted_changes
        if any(item.get('new_path', '').endswith(ext) for ext in supported_extensions)
    ]
    logger.info(f"After filtering by extension: {filtered_changes}")
    return filtered_changes


class PullRequestHandler:
    def __init__(self, webhook_data: dict, github_token: str, github_url: str):
        self.pull_request_number = None
        self.webhook_data = webhook_data
        self.github_token = github_token
        self.github_url = github_url
        self.event_type = None
        self.repo_full_name = None
        self.action = None
        self.parse_event_type()

    def parse_event_type(self):
        self.event_type = 'pull_request'
        self.parse_pull_request_event()

    def parse_pull_request_event(self):
        self.pull_request_number = self.webhook_data.get('pull_request', {}).get('number')
        self.repo_full_name = self.webhook_data.get('repository', {}).get('full_name')
        self.action = self.webhook_data.get('action')

    def get_pull_request_changes(self) -> list:
        if self.event_type != 'pull_request':
            logger.warn(f"Invalid event type: {self.event_type}. Only 'pull_request' event is supported now.")
            return []

        max_retries = 3
        retry_delay = 10
        for attempt in range(max_retries):
            url = f"https://api.github.com/repos/{self.repo_full_name}/pulls/{self.pull_request_number}/files"
            headers = {
                'Authorization': f'token {self.github_token}',
                'Accept': 'application/vnd.github.v3+json'
            }
            response = platform_get(url, headers=headers)
            logger.debug(
                f"Get changes response from GitHub (attempt {attempt + 1}): {response.status_code}, {response.text}, URL: {url}")

            if response.status_code == 200:
                files = response.json()
                if files:
                    changes = []
                    for file in files:
                        change = {
                            'old_path': file.get('filename'),
                            'new_path': file.get('filename'),
                            'diff': file.get('patch', ''),
                            'additions': file.get('additions', 0),
                            'deletions': file.get('deletions', 0)
                        }
                        changes.append(change)
                    return changes
                else:
                    logger.info(
                        f"Changes is empty, retrying in {retry_delay} seconds... (attempt {attempt + 1}/{max_retries}), URL: {url}")
                    time.sleep(retry_delay)
            else:
                logger.warn(f"Failed to get changes from GitHub (URL: {url}): {response.status_code}, {response.text}")
                return []

        logger.warning(f"Max retries ({max_retries}) reached. Changes is still empty.")
        return []

    def get_pull_request_commits(self) -> list:
        if self.event_type != 'pull_request':
            return []

        url = f"https://api.github.com/repos/{self.repo_full_name}/pulls/{self.pull_request_number}/commits"
        headers = {
            'Authorization': f'token {self.github_token}',
            'Accept': 'application/vnd.github.v3+json'
        }
        response = platform_get(url, headers=headers)
        logger.debug(f"Get commits response from GitHub: {response.status_code}, {response.text}")
        
        if response.status_code == 200:
            github_commits = response.json()
            gitlab_format_commits = []
            for commit in github_commits:
                gitlab_commit = {
                    'id': commit.get('sha'),
                    'title': commit.get('commit', {}).get('message', '').split('\n')[0],
                    'message': commit.get('commit', {}).get('message', ''),
                    'author_name': commit.get('commit', {}).get('author', {}).get('name'),
                    'author_email': commit.get('commit', {}).get('author', {}).get('email'),
                    'created_at': commit.get('commit', {}).get('author', {}).get('date'),
                    'web_url': commit.get('html_url')
                }
                gitlab_format_commits.append(gitlab_commit)
            return gitlab_format_commits
        else:
            logger.warn(f"Failed to get commits: {response.status_code}, {response.text}")
            return []

    def add_pull_request_notes(self, review_result):
        url = f"https://api.github.com/repos/{self.repo_full_name}/issues/{self.pull_request_number}/comments"
        headers = {
            'Authorization': f'token {self.github_token}',
            'Accept': 'application/vnd.github.v3+json'
        }
        data = {
            'body': review_result
        }
        response = platform_post(url, headers=headers, json=data)
        logger.debug(f"Add comment to GitHub PR {url}: {response.status_code}, {response.text}")
        if response.status_code == 201:
            logger.info("Comment successfully added to pull request.")
        else:
            logger.error(f"Failed to add comment: {response.status_code}")
            logger.error(response.text)

    def add_pull_request_inline_notes(self, comments: list, commit_id: str) -> int:
        """Post structured review issues as PR inline comments. Returns count posted."""
        if not comments or not commit_id:
            return 0
        from biz.scr.env import getenv
        if getenv("SCR_INLINE_COMMENTS_ENABLED", "1") != "1":
            return 0

        from biz.review.inline import format_inline_body

        url = f"https://api.github.com/repos/{self.repo_full_name}/pulls/{self.pull_request_number}/comments"
        headers = {
            'Authorization': f'token {self.github_token}',
            'Accept': 'application/vnd.github.v3+json'
        }
        posted = 0
        for comment in comments:
            new_line = getattr(comment, 'start_line', 0)
            path = getattr(comment, 'path', '')
            if new_line <= 0 or not path:
                continue
            data = {
                'body': format_inline_body(comment),
                'commit_id': commit_id,
                'path': path,
                'line': new_line,
                'side': 'RIGHT',
            }
            try:
                resp = platform_post(url, headers=headers, json=data)
                if resp.status_code in (200, 201):
                    posted += 1
                else:
                    logger.info(f"Inline comment skipped ({path}:{new_line}): {resp.status_code}, {resp.text}")
            except Exception as e:
                logger.warning(f"Inline comment error ({path}:{new_line}): {e}")
        logger.info(f"PR inline comments posted: {posted}/{len(comments)}")
        return posted

    def target_branch_protected(self) -> bool:
        url = f"https://api.github.com/repos/{self.repo_full_name}/branches?protected=true"
        headers = {
            'Authorization': f'token {self.github_token}',
            'Accept': 'application/vnd.github.v3+json'
        }

        response = platform_get(url, headers=headers)
        if response.status_code == 200:
            data = response.json()
            target_branch = self.webhook_data['pull_request']['base']['ref']
            return any(fnmatch.fnmatch(target_branch, item['name']) for item in data)
        else:
            logger.warn(f"Failed to get protected branches: {response.status_code}, {response.text}")
            return False


class PushHandler:
    def __init__(self, webhook_data: dict, github_token: str, github_url: str):
        self.webhook_data = webhook_data
        self.github_token = github_token
        self.github_url = github_url
        self.event_type = None
        self.repo_full_name = None
        self.branch_name = None
        self.commit_list = []
        self.parse_event_type()

    def parse_event_type(self):
        self.event_type = 'push'
        self.parse_push_event()

    def parse_push_event(self):
        self.repo_full_name = self.webhook_data.get('repository', {}).get('full_name')
        self.branch_name = self.webhook_data.get('ref', '').replace('refs/heads/', '')
        self.commit_list = self.webhook_data.get('commits', [])

    def get_push_commits(self) -> list:
        if self.event_type != 'push':
            logger.warn(f"Invalid event type: {self.event_type}. Only 'push' event is supported now.")
            return []

        commit_details = []
        for commit in self.commit_list:
            commit_info = {
                'message': commit.get('message'),
                'author': commit.get('author', {}).get('name'),
                'timestamp': commit.get('timestamp'),
                'url': commit.get('url'),
            }
            commit_details.append(commit_info)

        logger.info(f"Collected {len(commit_details)} commits from push event.")
        return commit_details

    def add_push_notes(self, message: str):
        if not self.commit_list:
            logger.warn("No commits found to add notes to.")
            return

        last_commit_id = self.commit_list[-1].get('id')
        if not last_commit_id:
            logger.error("Last commit ID not found.")
            return

        url = f"https://api.github.com/repos/{self.repo_full_name}/commits/{last_commit_id}/comments"
        headers = {
            'Authorization': f'token {self.github_token}',
            'Accept': 'application/vnd.github.v3+json'
        }
        data = {
            'body': message
        }
        response = platform_post(url, headers=headers, json=data)
        logger.debug(f"Add comment to commit {last_commit_id}: {response.status_code}, {response.text}")
        if response.status_code == 201:
            logger.info("Comment successfully added to push commit.")
        else:
            logger.error(f"Failed to add comment: {response.status_code}")
            logger.error(response.text)

    def __repository_commits(self, sha: str = "", per_page: int = 100, page: int = 1):
        url = f"https://api.github.com/repos/{self.repo_full_name}/commits?sha={sha}&per_page={per_page}&page={page}"
        headers = {
            'Authorization': f'token {self.github_token}',
            'Accept': 'application/vnd.github.v3+json'
        }
        response = platform_get(url, headers=headers)
        logger.debug(
            f"Get commits response from GitHub for repository_commits: {response.status_code}, {response.text}, URL: {url}")

        if response.status_code == 200:
            return response.json()
        else:
            logger.warn(
                f"Failed to get commits for sha {sha}: {response.status_code}, {response.text}")
            return []

    def get_parent_commit_id(self, commit_id: str) -> str:
        url = f"https://api.github.com/repos/{self.repo_full_name}/commits/{commit_id}"
        headers = {
            'Authorization': f'token {self.github_token}',
            'Accept': 'application/vnd.github.v3+json'
        }
        response = platform_get(url, headers=headers)
        logger.debug(
            f"Get commit response from GitHub: {response.status_code}, {response.text}, URL: {url}")

        if response.status_code == 200 and response.json().get('parents'):
            return response.json().get('parents')[0].get('sha', '')
        return ""

    def repository_compare(self, base: str, head: str):
        url = f"https://api.github.com/repos/{self.repo_full_name}/compare/{base}...{head}"
        headers = {
            'Authorization': f'token {self.github_token}',
            'Accept': 'application/vnd.github.v3+json'
        }
        response = platform_get(url, headers=headers)
        logger.debug(
            f"Get changes response from GitHub for repository_compare: {response.status_code}, {response.text}, URL: {url}")

        if response.status_code == 200:
            files = response.json().get('files', [])
            diffs = []
            for file in files:
                diff = {
                    'old_path': file.get('filename'),
                    'new_path': file.get('filename'),
                    'diff': file.get('patch', ''),
                    'status': file.get('status', ''),
                    'additions': file.get('additions', 0),
                    'deletions': file.get('deletions', 0),
                }
                diffs.append(diff)
            return diffs
        else:
            logger.warn(
                f"Failed to get changes for repository_compare: {response.status_code}, {response.text}")
            return []

    def get_push_changes(self) -> list:
        if self.event_type != 'push':
            logger.warn(f"Invalid event type: {self.event_type}. Only 'push' event is supported now.")
            return []

        if not self.commit_list:
            logger.info("No commits found in push event.")
            return []

        before = self.webhook_data.get('before', '')
        after = self.webhook_data.get('after', '')
        if before and after:
            if self.webhook_data.get('created', False):
                first_commit_id = self.commit_list[0].get('id')
                if first_commit_id:
                    parent_commit_id = self.get_parent_commit_id(first_commit_id)
                    if parent_commit_id:
                        before = parent_commit_id
            elif self.webhook_data.get('deleted', False):
                return []
            
            return self.repository_compare(before, after)
        else:
            logger.info("before or after not found in webhook data, trying to get changes from commits.")
            
            changes = []
            for commit in self.commit_list:
                commit_id = commit.get('id')
                if commit_id:
                    parent_id = self.get_parent_commit_id(commit_id)
                    if parent_id:
                        commit_changes = self.repository_compare(parent_id, commit_id)
                        changes.extend(commit_changes)
            
            return changes 
