import pandas as pd
import matplotlib.pyplot as plt
from sys import stderr

data_dir = "csf_benchmark_results"

def add_to_plot(filename, label=None, params={}):
    try:
        d = pd.read_csv(f"{data_dir}/{filename}.csv", sep=' ')
    except FileNotFoundError:
        print(f"{filename}.csv not found and thus skipped", file=stderr)
        return
    if label is None: label = filename
    for f, v in params.items():
        d = d[d[f] == v]
        label += f' {f}={v}'
    #best_idx = d.groupby(['input_len', 'entropy'])['bits/entry'].transform(min) == d['bits/entry']
    #print(d[best_idx])
    d[label] = d['bits/entry'] - d['entropy']
    d = d.groupby(['entropy'])[label].min()
    d.plot(legend=True)

#plt.plot([0, 1, 2, 3, 4, 5, 6, 7, 8])
add_to_plot('ble')
add_to_plot('fp_equal')
add_to_plot('fpgo_equal', params={'bits/seed': 4, 'bits/group': 16})
add_to_plot('fpgo_equal', params={'bits/seed': 8, 'bits/group': 32})
add_to_plot('ls_equal')
plt.show() 