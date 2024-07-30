import sys

def print_help():
    halp = """Binary writer
Usage:
    python3 create_corpus.py file_name "[0x69, 0x42, 0x00]" # hex
    python3 create_corpus.py file_name "[0, 12, 255]" # dec
    python3 create_corpus.py file_name "0x127381293792173" # hex concatenated
    """
    print(halp)

def print_error(err: str):
    print("Error:")
    print(err)
    print("")
    print_help()
    sys.exit(0)

def execute(file_name: str, b: bytearray):
    with open(file_name, "wb") as f:
        f.write(b)

args = sys.argv[1:]
if len(args) < 2:
    print_error("Not enough args!")

def main():
    file_name = args[0]
    str_binary = args[1]

    b = bytearray()
    if str_binary.startswith("0x"):
        b = bytearray.fromhex(str_binary[2:])
    else:
        if str_binary[0] != '[' or str_binary[-1] != ']':
            print_error("Invalid list bracket")

        str_binary = str_binary[1:-1]
        str_binary = str_binary.replace(' ', '')
        binary_elements = str_binary.split(',')

        if all(el.startswith("0x") for el in binary_elements):
            [b.extend(int(el, 16).to_bytes(1, 'big')) for el in binary_elements]
        elif all(not el.startswith("0x") for el in binary_elements):
            str_binary = "".join(binary_elements)
            [b.extend(int(el).to_bytes(1, 'big')) for el in binary_elements]
        else:
            print_error("only use hex or dec please")

    execute(file_name, b)

if __name__ == "__main__":
    main()
