let absolute = true;

const series = {};

async function run() {
  const response = await fetch('./data.json');
  const json = await response.json();

  for (let day of json) {
    for (let bm of day.data) {
      if (series[bm.name] === undefined)
        series[bm.name] = {};
      const data = series[bm.name];

      for (let output of bm.outputs) {
        if (data[output.name] === undefined)
          data[output.name] = { inputs: [], data: [] };
        data[output.name].inputs.push(bm.inputs);
        data[output.name].data.push([Date.parse(day.date), output.bytes]);
      }
    }
  }

  const body = document.body;
  for (let bm in series) {
    const element = document.createElement('div');
    element.id = bm;
    body.appendChild(element);
  }

  const button = document.getElementById('absolute');
  absolute = button.checked;
  button.onclick = function() {
    absolute = button.checked;
    render();
  };
  document.getElementById('controls').style.display = 'block';

  render();

  document.getElementById('loading').remove();
}

function render() {
  for (let bm in series) {
    const data = [];
    for (let output in series[bm]) {
      let sizes = series[bm][output];
      let raw = [];
      for (let datapoint of sizes.data) {
        let value = 0;
        if (absolute) {
          value = datapoint[1];
        } else {
          value = datapoint[1] / sizes[0][1];
        }
        raw.push([datapoint[0], value]);
      }
      data.push({ name: output, data: raw });
    }

    var myChart = Highcharts.chart(bm, {
      title: {
        text: bm,
      },
      yAxis: {
        title: {
          text: 'Size (in bytes)',
        },
        labels: {
          formatter: function() { return format(this.value); },
        },
      },
      xAxis: {
        type: 'datetime',
        dateTimeLabelFormats: {
          month: '%e. %b',
          year: '%b',
        },
        title: {
          text: 'Date',
        },
      },
      tooltip: {
        useHTML: true,
        headerFormat: '<b>{series.name}</b><br>',
        pointFormatter: function() {
          let text = format(this.y);
          if (this.index > 0) {
            const prev = this.series.data[this.index - 1];

            const inputs = point => {
              const data = series[point.series.chart.title.textStr];
              const inputs = data[point.series.name].inputs[point.index];
              return inputs;
            };

            const before = inputs(prev);
            const after = inputs(this);
            const diff = diff_inputs(before, after);
            text += '<br>' + diff;
          }
          return text;
        },
      },
      series: data,
      // plotOptions: {
      //   series: {
      //     point: {
      //       events: {
      //         click: function(event) {
      //           const point = event.point;
      //           if (point.index === 0) {
      //             return;
      //           }
      //           console.log(point.series.chart.title.textStr);
      //           console.log(point.series.name);
      //           console.log(point.x, point.y);
      //           const prev = point.series.data[point.index - 1];
      //           console.log(event);
      //         },
      //       },
      //     },
      //   },
      // },
    });
  }
}

function format(val) {
  if (absolute) {
    return bytes(val);
  }

  const pct = Math.round(val * 10000) / 100;
  return `${pct}%`;
}

function bytes(val) {
  if (val < 1024) {
    return `${val}B`;
  }
  val = Math.round((val * 100) / 1024) / 100;
  if (val < 1024) {
    return `${val}KB`;
  }
  val = Math.round((val * 100) / 1024) / 100;
  if (val < 1024) {
    return `${val}MB`;
  }
  val = Math.round((val * 100) / 1024) / 100;
  return `${val}GB`;
}

document.addEventListener('DOMContentLoaded', run);

function diff_inputs(a, b) {
  let text = '<ul>';

  const b_used = [];
  for (let i = 0; i < b.length; i++)
    b_used.push(false);

  for (let a_input of a) {
    for (let i = 0; i < b.length; i++) {
      if (b_used[i] || b[i].type != a_input.type)
        continue;
      b_used[i] = true;
      if (a_input.type == 'git') {
      } else if (a_input.type == 'wasm-pack') {
      } else if (a_input.type == 'rustc') {
        if (a_input.rev == b[i].rev)
          continue;

        const url = `https://github.com/rust-lang/rust/compare/${a_input.rev}...${b[i].rev}`;
        text += ` &middot; <a href="${url}">rustc changes</a>`;
      } else if (a_input.type == 'cargo-lock') {
      } else {
        throw new Error('unknown input type ' + a_input.type);
      }
      console.log(a_input);
    }
  }

  text += '</ul>';

  return text;
}
