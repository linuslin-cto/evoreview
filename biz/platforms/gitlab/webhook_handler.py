import os
import re
import time
from urllib.parse import urljoin
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

    filter_deleted_files_changes = [change for change in changes if not change.get("deleted_file")]

    filtered_changes = [
        {
            'diff': item.get('diff', ''),
            'new_path': item['new_path'],
            'additions': len(re.findall(r'^\+(?!\+\+)', item.get('diff', ''), re.MULTILINE)),
            'deletions': len(re.findall(r'^-(?!--)', item.get('diff', ''), re.MULTILINE))
        }
        for item in filter_deleted_files_changes
        if 'new_path' in item and any(item['new_path'].endswith(ext) for ext in supported_extensions)
    ]
    return filtered_changes


def slugify_url(original_url: str) -> str:
    """Turn a URL into a filesystem-safe slug."""
    original_url = re.sub(r'^https?://', '', original_url)
    target = re.sub(r'[^a-zA-Z0-9]', '_', original_url)
    target = target.rstrip('_')
    return target


class MergeRequestHandler:
    def __init__(self, webhook_data: dict, gitlab_token: str, gitlab_url: str):
        self.merge_request_iid = None
        self.webhook_data = webhook_data
        self.gitlab_token = gitlab_token
        self.gitlab_url = gitlab_url
        self.event_type = None
        self.project_id = None
        self.action = None
        self.parse_event_type()

    def parse_event_type(self):
        self.event_type = self.webhook_data.get('object_kind', None)
        if self.event_type == 'merge_request':
            self.parse_merge_request_event()

    def parse_merge_request_event(self):
        merge_request = self.webhook_data.get('object_attributes', {})
        self.merge_request_iid = merge_request.get('iid')
        self.project_id = merge_request.get('target_project_id')
        self.action = merge_request.get('action')

    def get_merge_request_changes(self) -> list:
        if self.event_type != 'merge_request':
            logger.warn(f"Invalid event type: {self.event_type}. Only 'merge_request' event is supported now.")
            return []

        max_retries = 3
        retry_delay = 10
        for attempt in range(max_retries):
            url = urljoin(f"{self.gitlab_url}/",
                          f"api/v4/projects/{self.project_id}/merge_requests/{self.merge_request_iid}/changes?access_raw_diffs=true")
            headers = {
                'Private-Token': self.gitlab_token
            }
            response = platform_get(url, headers=headers)
            logger.debug(
                f"Get changes response from GitLab (attempt {attempt + 1}): {response.status_code}, {response.text}, URL: {url}")

            if response.status_code == 200:
                changes = response.json().get('changes', [])
                if changes:
                    return changes
                logger.info(
                    f"Changes is empty, retrying in {retry_delay} seconds... (attempt {attempt + 1}/{max_retries}), URL: {url}")
                time.sleep(retry_delay)
            elif response.status_code in (429, 500, 502, 503, 504) and attempt < max_retries - 1:
                logger.warn(
                    f"API error {response.status_code} from GitLab, retrying in {retry_delay}s "
                    f"(attempt {attempt + 1}/{max_retries}), URL: {url}")
                time.sleep(retry_delay)
            else:
                logger.warn(f"Failed to get changes from GitLab (URL: {url}): {response.status_code}, {response.text}")
                return []

        logger.warning(f"Max retries ({max_retries}) reached. Changes is still empty.")
        return []

    def get_merge_request_commits(self) -> list:
        if self.event_type != 'merge_request':
            return []

        url = urljoin(f"{self.gitlab_url}/",
                      f"api/v4/projects/{self.project_id}/merge_requests/{self.merge_request_iid}/commits")
        headers = {
            'Private-Token': self.gitlab_token
        }
        response = platform_get(url, headers=headers)
        logger.debug(f"Get commits response from gitlab: {response.status_code}, {response.text}")
        if response.status_code == 200:
            return response.json()
        else:
            logger.warn(f"Failed to get commits: {response.status_code}, {response.text}")
            return []

    def add_merge_request_notes(self, review_result):
        url = urljoin(f"{self.gitlab_url}/",
                      f"api/v4/projects/{self.project_id}/merge_requests/{self.merge_request_iid}/notes")
        headers = {
            'Private-Token': self.gitlab_token,
            'Content-Type': 'application/json'
        }
        data = {
            'body': review_result
        }
        response = platform_post(url, headers=headers, json=data)
        logger.debug(f"Add notes to gitlab {url}: {response.status_code}, {response.text}")
        if response.status_code == 201:
            logger.info("Note successfully added to merge request.")
        else:
            logger.error(f"Failed to add note: {response.status_code}")
            logger.error(response.text)

    def get_merge_request_info(self) -> dict:
        """MR details including diff_refs (base/start/head SHA) for inline comments."""
        url = urljoin(f"{self.gitlab_url}/",
                      f"api/v4/projects/{self.project_id}/merge_requests/{self.merge_request_iid}")
        headers = {'Private-Token': self.gitlab_token}
        response = platform_get(url, headers=headers)
        if response.status_code == 200:
            return response.json()
        logger.warn(f"Failed to get merge request info: {response.status_code}, {response.text}")
        return {}

    def add_merge_request_inline_notes(self, comments: list) -> int:
        """Post structured review issues as MR inline discussions. Returns count posted."""
        if not comments:
            return 0
        from biz.scr.env import getenv
        if getenv("SCR_INLINE_COMMENTS_ENABLED", "1") != "1":
            return 0

        info = self.get_merge_request_info()
        diff_refs = info.get('diff_refs') or {}
        base_sha = diff_refs.get('base_sha')
        start_sha = diff_refs.get('start_sha')
        head_sha = diff_refs.get('head_sha')
        if not (base_sha and start_sha and head_sha):
            logger.warning("MR diff_refs 缺失，跳过行级 inline 评论。")
            return 0

        from biz.review.inline import format_inline_body

        url = urljoin(f"{self.gitlab_url}/",
                      f"api/v4/projects/{self.project_id}/merge_requests/{self.merge_request_iid}/discussions")
        headers = {'Private-Token': self.gitlab_token, 'Content-Type': 'application/json'}
        posted = 0
        for comment in comments:
            new_line = getattr(comment, 'start_line', 0)
            path = getattr(comment, 'path', '')
            if new_line <= 0 or not path:
                continue
            data = {
                'body': format_inline_body(comment),
                'position': {
                    'base_sha': base_sha,
                    'start_sha': start_sha,
                    'head_sha': head_sha,
                    'position_type': 'text',
                    'new_path': path,
                    'old_path': path,
                    'new_line': new_line,
                },
            }
            try:
                resp = platform_post(url, headers=headers, json=data)
                if resp.status_code in (200, 201):
                    posted += 1
                else:
                    # line outside diff range — skip; issue remains in summary note
                    logger.info(f"Inline note skipped ({path}:{new_line}): {resp.status_code}, {resp.text}")
            except Exception as e:
                logger.warning(f"Inline note error ({path}:{new_line}): {e}")
        logger.info(f"MR inline notes posted: {posted}/{len(comments)}")
        return posted

    def target_branch_protected(self) -> bool:
        url = urljoin(f"{self.gitlab_url}/",
                      f"api/v4/projects/{self.project_id}/protected_branches")
        headers = {
            'Private-Token': self.gitlab_token,
            'Content-Type': 'application/json'
        }
        response = platform_get(url, headers=headers)
        logger.debug(f"Get protected branches response from gitlab: {response.status_code}, {response.text}")
        if response.status_code == 200:
            data = response.json()
            target_branch = self.webhook_data['object_attributes']['target_branch']
            return any(fnmatch.fnmatch(target_branch, item.get('name', '')) for item in data)
        else:
            logger.warn(f"Failed to get protected branches: {response.status_code}, {response.text}")
            return False


class PushHandler:
    def __init__(self, webhook_data: dict, gitlab_token: str, gitlab_url: str):
        self.webhook_data = webhook_data
        self.gitlab_token = gitlab_token
        self.gitlab_url = gitlab_url
        self.event_type = None
        self.project_id = None
        self.branch_name = None
        self.commit_list = []
        self.parse_event_type()

    def parse_event_type(self):
        self.event_type = self.webhook_data.get('event_name', None)
        if self.event_type == 'push':
            self.parse_push_event()

    def parse_push_event(self):
        self.project_id = self.webhook_data.get('project_id', None)
        if self.project_id is None:
            self.project_id = self.webhook_data.get('project', {}).get('id')
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

        url = urljoin(f"{self.gitlab_url}/",
                      f"api/v4/projects/{self.project_id}/repository/commits/{last_commit_id}/comments")
        headers = {
            'Private-Token': self.gitlab_token,
            'Content-Type': 'application/json'
        }
        data = {
            'note': message
        }
        response = platform_post(url, headers=headers, json=data)
        logger.debug(f"Add comment to commit {last_commit_id}: {response.status_code}, {response.text}")
        if response.status_code == 201:
            logger.info("Comment successfully added to push commit.")
        else:
            logger.error(f"Failed to add comment: {response.status_code}")
            logger.error(response.text)

    def __repository_commits(self, ref_name: str = "", since: str = "", until: str = "", pre_page: int = 100,
                             page: int = 1):
        url = urljoin(f'{self.gitlab_url}/', f'api/v4/projects/{self.project_id}/repository/commits')
        headers = {
            'Private-Token': self.gitlab_token
        }
        params = {
            'ref_name': ref_name,
            'since': since,
            'until': until,
            'per_page': pre_page,
            'page': page,
        }
        response = platform_get(url, headers=headers, params=params)
        logger.debug(
            f"Get commits response from GitLab for repository_commits: {response.status_code}, {response.text}, URL: {url}")

        if response.status_code == 200:
            return response.json()
        else:
            logger.warn(
                f"Failed to get commits for ref {ref_name}: {response.status_code}, {response.text}")
            return []

    def repository_compare(self, before: str, after: str):
        url = f"{urljoin(f'{self.gitlab_url}/', f'api/v4/projects/{self.project_id}/repository/compare')}?from={before}&to={after}"
        headers = {
            'Private-Token': self.gitlab_token
        }
        response = platform_get(url, headers=headers)
        logger.debug(
            f"Get changes response from GitLab for repository_compare: {response.status_code}, {response.text}, URL: {url}")

        if response.status_code == 200:
            return response.json().get('diffs', [])
        else:
            logger.warn(
                f"Failed to get changes for repository_compare: {response.status_code}, {response.text}")
            return []

    def get_commit_diff(self, commit_sha: str):
        url = f"{urljoin(f'{self.gitlab_url}/', f'api/v4/projects/{self.project_id}/repository/commits/{commit_sha}/diff')}"
        headers = {
            'Private-Token': self.gitlab_token
        }
        response = platform_get(url, headers=headers)
        logger.debug(
            f"Get commit diff response from GitLab: {response.status_code}, {response.text}, URL: {url}")

        if response.status_code == 200:
            return response.json()
        else:
            logger.warn(
                f"Failed to get commit diff for {commit_sha}: {response.status_code}, {response.text}")
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
        
        if not before or not after:
            logger.warn("Missing before or after commit SHA in webhook data.")
            return []

        if after.startswith('0000000'):
            logger.info("Branch deletion detected, no changes to review.")
            return []
        
        if before.startswith('0000000'):
            logger.info("New branch creation detected, using commit diff API.")
            if self.commit_list:
                latest_commit_id = after
                return self.get_commit_diff(latest_commit_id)
            else:
                return []
        else:
            logger.info(f"Comparing commits from {before} to {after}")
            return self.repository_compare(before, after)
