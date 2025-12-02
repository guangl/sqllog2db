"""验证 JSONL 文件的脚本"""
import json
import sys

def verify_jsonl(file_path, num_lines=10):
    """验证 JSONL 文件格式并显示前几行"""
    print(f"验证 JSONL 文件: {file_path}\n")
    
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            total_lines = 0
            valid_lines = 0
            
            print(f"前 {num_lines} 行数据:\n")
            
            for i, line in enumerate(f, 1):
                total_lines = i
                try:
                    data = json.loads(line.strip())
                    valid_lines += 1
                    
                    if i <= num_lines:
                        print(f"行 {i}:")
                        print(json.dumps(data, indent=2, ensure_ascii=False))
                        print()
                        
                except json.JSONDecodeError as e:
                    if i <= num_lines:
                        print(f"❌ 行 {i} 解析失败: {e}")
                
                # 每处理 100 万行显示进度
                if i % 1000000 == 0:
                    print(f"已处理 {i:,} 行...")
            
            print(f"\n✓ 验证完成!")
            print(f"总行数: {total_lines:,}")
            print(f"有效 JSON 行: {valid_lines:,}")
            print(f"无效行: {total_lines - valid_lines:,}")
            print(f"有效率: {valid_lines/total_lines*100:.2f}%")
            
    except FileNotFoundError:
        print(f"❌ 文件不存在: {file_path}")
        sys.exit(1)
    except Exception as e:
        print(f"❌ 错误: {e}")
        sys.exit(1)

if __name__ == "__main__":
    file_path = sys.argv[1] if len(sys.argv) > 1 else "export/sqllog2db.jsonl"
    num_lines = int(sys.argv[2]) if len(sys.argv) > 2 else 3
    verify_jsonl(file_path, num_lines)
