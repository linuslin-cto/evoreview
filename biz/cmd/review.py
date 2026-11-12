"""Command-line interface for codebase review functionalities."""
from pathlib import Path

from dotenv import load_dotenv

from biz.cmd.func.branch import BranchReviewFunc
from biz.cmd.func.complexity import ComplexityReviewFunc
from biz.cmd.func.diff_review import DiffReviewFunc
from biz.cmd.func.directory import DirectoryReviewFunc
from biz.cmd.func.mysql import MySQLReviewFunc


def welcome_message():
    print("\n欢迎使用 Codebase Review 工具！\n")


def get_func_choice():
    options = {
        "1": ("本地 Diff 审查（提交前自审 / commit / 范围）", DiffReviewFunc),
        "2": ("Review 目录结构规范", DirectoryReviewFunc),
        "3": ("Review 代码分支命名规范", BranchReviewFunc),
        "4": ("Review 代码复杂度", ComplexityReviewFunc),
        "5": ("Review MySQL 数据库表结构", MySQLReviewFunc),
    }

    print("📌 请选择功能:")
    for key, (desc, _) in options.items():
        print(f"{key}. {desc}")

    while True:
        choice = input("请输入数字 (1-5): ").strip()
        if choice in options:
            return options[choice][1]
        print("❌ 无效的选择，请输入 1-5")


def main():
    env_path = Path(__file__).resolve().parents[2] / "conf" / ".env"
    load_dotenv(env_path if env_path.is_file() else "conf/.env")
    welcome_message()

    FuncClass = get_func_choice()
    func = FuncClass()
    func.process()


if __name__ == "__main__":
    main()
