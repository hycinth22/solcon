# 关闭ASLR后，PIE也无法生效
sudo sh -c 'echo 0 > /proc/sys/kernel/randomize_va_space'
