import pandas as pd
import matplotlib.pyplot as plt
from sys import stderr
from bs4 import BeautifulSoup

data_dir = "."

def get_data(filename):
    '''Try to read data from file and returns them or None.'''
    try:
        d = pd.read_csv(f"{data_dir}/{filename}.csv", sep=',')
        if d.empty:
            print(f"no data {filename}.csv")
            return None
        return d
    except FileNotFoundError:
        print(f"{filename}.csv not found and thus skipped", file=stderr)
        return None

def exp_str(n: int):
    return f'10<sup>{len(str(n))-1}</sup>'

def process_alg_name(n: str):
    n = n.replace(' over ranks', '')
    if n.startswith('bitm') or n.startswith('succinct') or n.startswith('sucds') or n.startswith('vers'):
        n += "*"
    return n

def gen_tab(filename, params={}, filter=None):
    d = get_data(filename)
    if d is None: return
    out_filename = filename
    for f, v in params.items():
        d = d[d[f] == v]
        out_filename += f'_{v}'
    out_filename += '.html'
    if d.empty:
        print(f"cannot construct {out_filename}, no needed data in {filename}.csv")
        return    
    if filename.startswith('select'): d["method"] = d["method"].apply(process_alg_name)
    d["percent of ones"] = (d["num"]*100/d["universe"]).round(0).astype(int)
    d["space overhead [%]"] = d["space_overhead"].round(1)
    d["time / query [ns]"] = d["time_per_query"].round(0).astype(int)
    if filter is not None: d = d[filter(d)]
    #d["universe"] = d["universe"].apply(exp_str)
    d.rename(columns={"universe": "bit vector length"}, inplace=True)
    d = d.pivot_table(index="method",
            values=["space overhead [%]", "time / query [ns]"],
            columns=["bit vector length", "percent of ones"],
            )
    #d = d.reorder_levels([1,0,2], axis=1)
    d = d.reorder_levels([1,2,0], axis=1)
    #print(d.to_markdown())
    #d.sort_index(axis=1, level=[1,2], ascending=True, inplace=True)
    d.sort_index(axis='columns', level=[0,1,2], ascending=[False, False, True], inplace=True)
    #d.sort_index(axis=1, level=[0,2,1], ascending=False, inplace=True)
    d.rename(exp_str, axis='columns', level=0, inplace=True)
    html = BeautifulSoup(d.to_html(escape=False, border=0), 'html.parser')
    for tr in html.find_all('tr'):        
        if tr.find('th', string='method'):
            tr.decompose()
    for e in html.find_all(halign=True): del e['halign']
    #for td in html.find_all('td'): td['style'] = 'text-align: center;'
    tab = html.find('table')
    del tab['class']
    tab['style'] = 'text-align: center;'
    for e in html.find_all('tr'):
        if e.find('th', string='space overhead [%]'): e['style'] = 'font-size: 75%;'
    for h in html.find('tbody').find_all('th'): h['style'] = 'font-size: 75%;'
    #for e in html.find_all('th', string='space overhead [%]'): e['style'] = 'font-size: smaller;'
    #for e in html.find_all('th', string="time / query [ns]"): e['style'] = 'font-size: smaller;'
    #print(d.to_markdown())
    with open(out_filename, "w", encoding = 'utf-8') as file: 
        file.write(str(html).replace('\n\n', '\n').replace('\n<th', '<th').replace('</th>\n', '</th>').replace('</td>\n', '</td>'))

for distribution in ('uniform', 'adversarial'):
    gen_tab('rank', filter=lambda d: d["percent of ones"] > 5, params={'distribution': distribution})
    gen_tab('select1', filter=lambda d: d["percent of ones"] > 5, params={'distribution': distribution})
    gen_tab('select0', filter=lambda d: d["percent of ones"] > 5, params={'distribution': distribution})
gen_tab('rank', filter=lambda d: d["percent of ones"] > 5, params={'distribution': 'uniform', 'universe':1000000000})
gen_tab('select1', filter=lambda d: d["percent of ones"] > 5, params={'distribution': 'uniform', 'universe':1000000000})
gen_tab('select0', filter=lambda d: d["percent of ones"] > 5, params={'distribution': 'uniform', 'universe':1000000000})