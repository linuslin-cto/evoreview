import os
from heapq import nlargest
from pathlib import Path

import lizard
from tqdm import tqdm

from biz.cmd.func.base import BaseReviewFunc


class ComplexityReviewFunc(BaseReviewFunc):
    """
    计算代码复杂度.
    """

    def __init__(self):
        super().__init__()
        self.directory = None
        self.top_n = 10

    def validate_directory(self, directory):
        """
        验证用户输入的目录是否存在。
        :param directory: 用户输入的目录路径
        :return: 如果目录存在返回 True，否则返回 False
        """
        try:
            return Path(directory).resolve().is_dir()
        except Exception:
            return False

    def parse_arguments(self):
        """
        使用交互方式获取用户输入的参数。
        """

        while True:
            try:
                self.directory = input("请输入项目的根目录路径: ").strip()
            except (KeyboardInterrupt, EOFError):
                print("\n操作已取消")
                raise SystemExit(0)
            if self.validate_directory(self.directory):
                break
            print("❌ 目录不存在，请输入有效路径")

        while True:
            try:
                top_n_input = input("请输入需要分析的复杂度最高文件数量（默认10）: ").strip()
            except (KeyboardInterrupt, EOFError):
                print("\n操作已取消")
                raise SystemExit(0)
            if not top_n_input:
                self.top_n = 10
                break
            try:
                self.top_n = int(top_n_input)
                if self.top_n > 0:
                    break
                print("❌ 请输入正整数")
            except ValueError:
                print("❌ 请输入有效数字")

    def find_most_complex_functions(self):
        try:
            analysis_result = lizard.analyze([self.directory])
        except Exception as e:
            print(f"❌ 分析目录失败: {e}")
            return []
        functions = []
        for file_info in tqdm(analysis_result, desc="分析文件", unit="file"):
            functions.extend(file_info.function_list)
        return nlargest(self.top_n, functions, key=lambda f: f.cyclomatic_complexity)

    def process(self):
        self.parse_arguments()
        top_functions = self.find_most_complex_functions()
        print("🔥 以下是最复杂的函数：")
        for func in top_functions:
            print(f"{func.name} (文件: {func.filename}, 复杂度: {func.cyclomatic_complexity})")


if __name__ == '__main__':
    ComplexityReviewFunc().process()
